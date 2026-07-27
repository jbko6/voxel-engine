use std::{slice, sync::Arc};

use raw_window_handle::{HasDisplayHandle};
use vulkano::{Version, VulkanError, VulkanLibrary, buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage}, device::{Device, DeviceCreateInfo, DeviceExtensions, DeviceFeatures, Queue, QueueCreateInfo, QueueFlags, physical::PhysicalDeviceType}, image::{ImageFormatInfo, ImageLayout, ImageUsage, view::ImageView}, instance::{Instance, InstanceCreateFlags, InstanceCreateInfo, InstanceExtensions}, memory::allocator::{AllocationCreateInfo, DeviceLayout, MemoryTypeFilter}, pipeline::{DynamicState, GraphicsPipeline, PipelineLayout, PipelineShaderStageCreateInfo, graphics::{GraphicsPipelineCreateInfo, color_blend::{ColorBlendAttachmentState, ColorBlendState}, input_assembly::InputAssemblyState, multisample::MultisampleState, rasterization::RasterizationState, vertex_input::{Vertex, VertexDefinition}, viewport::{Viewport, ViewportState}}}, render_pass::Subpass, shader::spirv::{ExecutionModel::MeshEXT, StorageClass::Image}, swapchain::{Surface, Swapchain, SwapchainCreateInfo}};
use vulkano_taskgraph::{ ClearValues, Id, QueueFamilyType, Task as VulkanTask, TaskContext, command_buffer::RecordingCommandBuffer, descriptor_set::{BindlessContext, StorageImageId}, graph::{AttachmentInfo, CompileInfo, ExecutableTaskGraph, ExecuteError, TaskGraph}, resource::{AccessTypes, Flight, HostAccessType, ImageLayoutType, Resources, ResourcesCreateInfo}, resource_map};
use winit::window::Window;

pub mod mesh;
pub use mesh::*;

const MAX_FRAMES_IN_FLIGHT: u32 = 2;
const MIN_SWAPCHAIN_IMAGES: u32 = MAX_FRAMES_IN_FLIGHT + 1;

pub struct Renderer {
    instance: Arc<Instance>,
    device: Arc<Device>,
    queue: Arc<Queue>,
    resources: Arc<Resources>,
    flight_id: Id<Flight>,
    rcx: Option<RenderContext>,
}

struct RenderContext {
    window: Arc<Window>,
    swapchain_id: Id<Swapchain>,
    viewport: Viewport,
    recreate_swapchain: bool,
    task_graph: ExecutableTaskGraph<Self>,
    virtual_swapchain_id: Id<Swapchain>,
    meshes: Vec<Mesh>,
}

impl Renderer {
    pub fn new(event_loop: &impl HasDisplayHandle) -> Self {

        // Initialize Vulkan instance
        let library = unsafe { VulkanLibrary::new() }.unwrap();
        let required_extensions = Surface::required_extensions(event_loop);
        let instance = Instance::new(
            &library,
            &InstanceCreateInfo {
                flags: InstanceCreateFlags::ENUMERATE_PORTABILITY,
                enabled_extensions: &required_extensions,
                ..Default::default()
            },
        ).unwrap();

        // Device requirements
        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            ..BindlessContext::required_extensions(&instance)
        };
        let device_features = DeviceFeatures {
            ..BindlessContext::required_features(&instance)
        };

        // Find device
        let physical_device = instance
            .enumerate_physical_devices()
            .unwrap()
            .filter(|p| p.api_version() > Version::V1_3)
            .filter(|p| {
                p.supported_extensions().contains(&device_extensions)
                    && p.supported_features().contains(&device_features)
            })
            .min_by_key(|p| match p.properties().device_type {
                PhysicalDeviceType::DiscreteGpu => 0,
                PhysicalDeviceType::IntegratedGpu => 1,
                PhysicalDeviceType::VirtualGpu => 2,
                PhysicalDeviceType::Cpu => 3,
                PhysicalDeviceType::Other => 4,
                _ => 5
            })
            .unwrap();

        println!(
            "Using device: {} (type: {:?})",
            physical_device.properties().device_name,
            physical_device.properties().device_type
        );
        
        // Find queue family (TODO: separate presentation)
        let queue_family_index = physical_device.queue_family_properties()
            .iter()
            .enumerate()
            .position(|(i, q)| {
                q.queue_flags
                    .contains(QueueFlags::GRAPHICS)
                    && physical_device.presentation_support(i as u32, event_loop)
            })
            .map(|i| i as u32)
            .unwrap();

        let (device, mut queues) = Device::new(
            &physical_device,
            &DeviceCreateInfo {
                enabled_extensions: &device_extensions,
                enabled_features: &device_features,
                queue_create_infos: &[
                    QueueCreateInfo {
                        queue_family_index,
                        ..Default::default()
                    }
                ],
                ..Default::default()
            }
        ).unwrap();

        let queue = queues.next().unwrap();

        let resources = Resources::new(
            &device,
            &ResourcesCreateInfo {
                bindless_context: Some(&Default::default()),
                ..Default::default()
            }
        ).unwrap();

        let flight_id = resources.create_flight(MAX_FRAMES_IN_FLIGHT).unwrap();

        Self {
            instance,
            device,
            queue,
            resources,
            flight_id,
            rcx: None
        }
    }

    pub fn set_meshes(&mut self, meshes: Vec<Mesh>) {
        self.rcx.as_mut().unwrap().meshes = meshes;
    }

    pub fn set_window(&mut self, window: Arc<winit::window::Window>) {
        let surface = Surface::from_window(&self.instance, &window).unwrap();
        let window_size = window.inner_size();
        
        let swapchain_format;
        let swapchain_id = {
            let surface_capabilities = self
                .device
                .physical_device()
                .surface_capabilities(&surface, &Default::default())
                .unwrap();

            (swapchain_format, _) = self
                .device
                .physical_device()
                .surface_formats(&surface, &Default::default())
                .unwrap()[0];

            self.resources
                .create_swapchain(
                    &surface, 
                &SwapchainCreateInfo {
                    min_image_count: surface_capabilities
                        .min_image_count
                        .max(MIN_SWAPCHAIN_IMAGES),
                    image_format: swapchain_format,
                    image_extent: window_size.into(),
                    image_usage: ImageUsage::COLOR_ATTACHMENT,
                    composite_alpha: surface_capabilities
                        .supported_composite_alpha
                        .into_iter()
                        .next()
                        .unwrap(),
                    ..Default::default()
                }
            ).unwrap()
        };

        let viewport = Viewport {
            offset: [0.0, 0.0],
            extent: window_size.into(),
            min_depth: 0.0,
            max_depth: 1.0,
        };

        let mut task_graph = TaskGraph::new(&self.resources);

        let virtual_swapchain_id = task_graph.add_swapchain(&SwapchainCreateInfo {
            image_format: swapchain_format,
            ..Default::default()
        });

        let virtual_framebuffer_id = task_graph.add_framebuffer();

        let render_node_id = task_graph
            .create_task_node(
                "Render", 
                QueueFamilyType::Graphics,
                RenderTask::new(virtual_swapchain_id)
            )
            .framebuffer(virtual_framebuffer_id)
            .color_attachment(
                virtual_swapchain_id.current_image_id(), 
                AccessTypes::COLOR_ATTACHMENT_WRITE, 
                ImageLayoutType::Optimal, 
                &AttachmentInfo {
                    clear: true,
                    ..Default::default()
                }
            ).build();

        let mut task_graph = unsafe {
            task_graph.compile(&CompileInfo {
                queues: &[&self.queue],
                present_queue: Some(&self.queue),
                flight_id: self.flight_id,
                ..Default::default()
            })
        }.unwrap();

        let render_node = task_graph.task_node_mut(render_node_id).unwrap();
        let subpass = render_node.subpass().unwrap().clone();
        render_node
            .task_mut()
            .downcast_mut::<RenderTask>()
            .unwrap()
            .create_pipeline(self, &subpass);

        let recreate_swapchain = false;
        
        self.rcx = Some(RenderContext { 
            window, 
            swapchain_id, 
            viewport, 
            recreate_swapchain, 
            task_graph, 
            meshes: Vec::new(),
            virtual_swapchain_id
        })        
    }

    pub fn recreate_swapchain(&mut self) {
        if let Some(rcx) = &mut self.rcx {
            rcx.recreate_swapchain = true;
        }
    }

    pub fn draw(&mut self) {
        let rcx = self.rcx.as_mut().unwrap();

        let window_size = rcx.window.inner_size();

        if window_size.width == 0 || window_size.height == 0 {
            return;
        }

        if rcx.recreate_swapchain {
            rcx.swapchain_id = self
                .resources
                .recreate_swapchain(rcx.swapchain_id, |create_info| SwapchainCreateInfo {
                    image_extent: window_size.into(),
                    ..create_info.clone()
                })
                .expect("failed");
            
            rcx.viewport.extent = window_size.into();
            rcx.recreate_swapchain = false;
        }

        let flight = self.resources.flight(self.flight_id);
        flight.wait(None).unwrap();

        let resource_map =
            resource_map!(&rcx.task_graph, rcx.virtual_swapchain_id => rcx.swapchain_id)
            .unwrap();

        match unsafe {
            rcx.task_graph.execute(resource_map, rcx, || rcx.window.pre_present_notify())
        } {
            Ok(()) => {}
            Err(ExecuteError::Swapchain { error: VulkanError::OutOfDate, .. }) => {
                rcx.recreate_swapchain = true;
            }
            Err(e) => {
                panic!("Failed to execute task graph: {:?}", e);
            }
        }
    }
}

struct RenderTask {
    pipeline: Option<Arc<GraphicsPipeline>>,
    swapchain_id: Id<Swapchain>,
}

impl RenderTask {
    fn new(swapchain_id: Id<Swapchain>) -> Self {
        Self {
            pipeline: None,
            swapchain_id,
        }
    }

    pub fn create_pipeline(&mut self, renderer: &Renderer, subpass: &Subpass) {
        // The next step is to create the shaders.
        //
        // The raw shader creation API provided by the vulkano library is unsafe for various
        // reasons, so the `shader!` macro provides a way to generate a Rust module from shader
        // source. In the example below, the source is provided as a string input directly to the
        // shader, but a path to a source file can be provided as well. Note that the user must
        // specify the type of shader (e.g., "vertex", "fragment", etc.) using the `ty` option of
        // the macro.
        //
        // The items generated by the `shader!` macro include a `load` function which loads the
        // shader using a logical device. The module also includes structs compatible with the ones
        // defined in the shader source, such as uniforms and push constants for example.
        //
        // A more detailed overview of what the `shader!` macro generates can be found in the
        // vulkano-shaders crate docs. You can view them at https://docs.rs/vulkano-shaders/
        mod vs {
            vulkano_shaders::shader! {
                ty: "vertex",
                root_path_env: "CARGO_MANIFEST_DIR",
                src: r"
                    #version 450

                    layout(location = 0) in vec3 position;
                    layout(location = 1) in vec3 normal;
                    layout(location = 2) in vec3 color;

                    void main() {
                        gl_Position = vec4(position, 1.0) + vec4(0.0, 0.0, 0.0, 0.0);
                    }
                ",
            }
        }

        mod fs {
            vulkano_shaders::shader! {
                ty: "fragment",
                root_path_env: "CARGO_MANIFEST_DIR",
                src: r"
                    #version 450

                    layout(location = 0) out vec4 f_color;

                    void main() {
                        f_color = vec4(251.0 / 255.0, 113.0 / 255.0, 133.0 / 255.0, 1.0);
                    }
                ",
            }
        }

        // Before we draw, we have to create what is called a "pipeline". A pipeline describes how
        // a GPU operation is to be performed. It is similar to an OpenGL program, but it also
        // contains many settings for customization, all baked into a single object. For drawing
        // triangles, we create a graphics pipeline, but there are also other types of pipelines.
        let pipeline = {
            // First, we load the shaders that the pipeline will use: the vertex shader and the
            // fragment shader.
            //
            // A Vulkan shader can in theory contain multiple entry points, so we have to specify
            // which one to use.
            let vs = unsafe { vs::load(&renderer.device) }
                .unwrap()
                .entry_point("main")
                .unwrap();
            let fs = unsafe { fs::load(&renderer.device) }
                .unwrap()
                .entry_point("main")
                .unwrap();

            // Automatically generate a vertex input state from the vertex shader's input
            // interface that takes a single vertex buffer containing `Vertex` structs.
            let vertex_input_state = MeshVertex::per_vertex().definition(&vs).unwrap();

            // Make a list of the shader stages that the pipeline will have.
            let stages = [
                PipelineShaderStageCreateInfo::new(&vs),
                PipelineShaderStageCreateInfo::new(&fs),
            ];

            // We must now create a "pipeline layout" object, which describes the locations and
            // types of descriptor sets and push constants used by the shaders in the pipeline.
            //
            // Multiple pipelines can share a common layout object, which is more efficient. The
            // shaders in a pipeline must use a subset of the resources described in its pipeline
            // layout, but the pipeline layout is allowed to contain resources that are not present
            // in the shaders; they can be used by shaders in other pipelines that share the same
            // layout. Thus, it is a good idea to design shaders so that many pipelines have common
            // resource locations, which allows them to share pipeline layouts.
            //
            // Since we only have one pipeline in this example, and thus one pipeline layout, we
            // automatically generate the layout from the resources used in the shaders. In a real
            // application, you would specify this information manually so that you can re-use one
            // layout in multiple pipelines.
            let layout = PipelineLayout::from_stages(&renderer.device, &stages).unwrap();

            // Finally, create the pipeline.
            GraphicsPipeline::new(
                &renderer.device,
                None,
                &GraphicsPipelineCreateInfo {
                    stages: &stages,
                    // How vertex data is read from the vertex buffers into the vertex shader.
                    vertex_input_state: Some(&vertex_input_state),
                    // How vertices are arranged into primitive shapes. The default primitive shape
                    // is a triangle.
                    input_assembly_state: Some(&InputAssemblyState::default()),
                    // How primitives are transformed and clipped to fit the framebuffer. We use a
                    // resizable viewport set to draw over the entire window.
                    viewport_state: Some(&ViewportState::default()),
                    // How polygons are culled and converted into a raster of pixels. The default
                    // value does not perform any culling.
                    rasterization_state: Some(&RasterizationState::default()),
                    // How multiple fragment shader samples are converted to a single pixel value.
                    // The default value does not perform any multisampling.
                    multisample_state: Some(&MultisampleState::default()),
                    // How pixel values are combined with the values already present in the
                    // framebuffer. The default value overwrites the old value with the new one
                    // without any blending.
                    color_blend_state: Some(&ColorBlendState {
                        attachments: &[ColorBlendAttachmentState::default()],
                        ..Default::default()
                    }),
                    // Dynamic state allows us to specify parts of the pipeline settings when
                    // recording the command buffer, before we perform drawing. Here, we specify
                    // that the viewport should be dynamic.
                    dynamic_state: &[DynamicState::Viewport],
                    // We have to indicate which subpass of which render pass this pipeline is
                    // going to be used in. The pipeline will only be usable from this particular
                    // subpass.
                    subpass: Some(subpass.into()),
                    ..GraphicsPipelineCreateInfo::new(&layout)
                },
            )
            .unwrap()
        };

        self.pipeline = Some(pipeline);
    }
}

impl VulkanTask for RenderTask {
    type World = RenderContext;

    fn clear_values(&self, clear_values: &mut ClearValues<'_>, _world: &Self::World) {
        clear_values.set(self.swapchain_id.current_image_id(), [0.0, 0.0, 0.0, 1.0]);
    }

    unsafe fn execute(
        &self,
        cbf: &mut RecordingCommandBuffer<'_>,
        _tcx: &mut TaskContext<'_>,
        rcx: &Self::World
    ) -> vulkano_taskgraph::TaskResult {
        unsafe {
            cbf.set_viewport(0, slice::from_ref(&rcx.viewport));
            
            cbf.bind_pipeline_graphics(self.pipeline.as_ref().unwrap());

            let meshes = &rcx.meshes;
            
            for mesh in meshes {
                cbf.bind_vertex_buffers(0, &[mesh.vertex_buffer_id], &[0], &[], &[]);
                cbf.bind_index_buffer(mesh.index_buffer_id, 0, Some(mesh.index_count as u64), vulkano::buffer::IndexType::U32);
                cbf.draw_indexed(mesh.index_count, 1, 0, 0, 0);
            }
        }

        Ok(())
    }
}
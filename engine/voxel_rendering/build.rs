use std::fs;
use std::path::Path;

fn visit_dirs<T>(dir: &Path, cb: &T) -> std::io::Result<()>
where
    T: Fn(&std::path::PathBuf, shaderc::ShaderKind),
{
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                visit_dirs(&path, cb)?;
            } else {
                let path_buf = entry.path();
                if let Some(shader_kind) = get_shader_kind(&path_buf) {
                    cb(&path_buf, shader_kind);
                }
            }
        }
    }

    Ok(())
}

fn get_shader_kind(path_buf: &std::path::PathBuf) -> Option<shaderc::ShaderKind> {
    let extension = path_buf
        .extension()
        .expect("file has no extension")
        .to_str()
        .expect("extension cannot be converted to &str");

    match extension {
        "vert" => Some(shaderc::ShaderKind::Vertex),
        "frag" => Some(shaderc::ShaderKind::Fragment),
        "tese" => Some(shaderc::ShaderKind::TessEvaluation),
        "tesc" => Some(shaderc::ShaderKind::TessControl),
        _ => None,
    }
}

fn compile_shader(path_buf: &std::path::PathBuf, shader_kind: shaderc::ShaderKind) {
    let shader_str = fs::read_to_string(path_buf)
        .expect(&format!("failed to read shader {:?} to string", path_buf));

    let compiler = shaderc::Compiler::new().expect("failed to create shader compilier");

    println!("compiling shader {:?}", path_buf);

    let mut options = shaderc::CompileOptions::new().expect("failed to create shader compile options");
    options.set_target_env(shaderc::TargetEnv::Vulkan, shaderc::EnvVersion::Vulkan1_3 as u32);
    options.set_target_spirv(shaderc::SpirvVersion::V1_6);

    let spv = compiler
        .compile_into_spirv(
            &shader_str,
            shader_kind,
            &path_buf.to_str().unwrap(),
            "main",
            Some(&options),
        )
        .expect(&format!("failed to compile shader {:?}", path_buf));

    let mut file_name = path_buf
        .file_name()
        .expect("shader file should have a name")
        .to_os_string();

    println!("cargo:rerun-if-changed={}", path_buf.display());

    file_name.push(".spv");

    let mut spv_path = path_buf
        .parent()
        .expect("failed to get shader file parent folder")
        .join("..")
        .join("..")
        .join("..")
        .join("shaders");

    std::fs::create_dir_all(spv_path.clone()).expect(&format!(
        "failed to create directory for shader {:?}",
        path_buf
    ));

    spv_path.push(file_name);

    fs::write(spv_path, spv.as_binary_u8()).expect("failed to write shader binary");
}

fn main() -> Result<(), i32> {
    let shaders_dir = Path::new("shaders");

    visit_dirs(shaders_dir, &compile_shader).expect("failed to visit shader directories");

    Ok(())
}

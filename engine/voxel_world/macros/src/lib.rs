use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_derive(Entity)]
pub fn entity_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_entity_macro(&ast)
}

fn impl_entity_macro(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let generated = quote! {
        impl Entity for #name {
            fn name(&self) -> &str {
                &self.name
            }
            fn position(&self) -> &Vec3<f32> {
                &self.position
            }
            fn rotation(&self) -> &Quaternion<f32> {
                &self.rotation
            }
            fn scale(&self) -> &Vec3<f32> {
                &self.scale
            }
        }
    };
    generated.into()
}
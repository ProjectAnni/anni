fn main() {
    cynic_codegen::register_schema("annim")
        .from_sdl_file("schemas/annim.graphql")
        .unwrap()
        .as_default()
        .unwrap();
}

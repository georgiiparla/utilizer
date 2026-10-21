fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=assets/utilizer.rc");
    println!("cargo:rerun-if-changed=assets/logo.ico");
    embed_resource::compile_for("assets/utilizer.rc", ["utilizer"], embed_resource::NONE)
        .manifest_required()?;
    Ok(())
}

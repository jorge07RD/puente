//! En Windows, incrusta el icono en `puente.exe`: lo usan el Explorador, la barra de
//! tareas y los accesos directos.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/puente.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../../assets/puente.ico");
        resource.set("ProductName", "Puente");
        resource.set("FileDescription", "Puente");
        resource.compile().expect("icono incrustado");
    }
}

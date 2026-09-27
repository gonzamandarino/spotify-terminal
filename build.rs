//! Ícono y metadatos de los `.exe` en Windows (spec 004). El ícono lo
//! genera `scripts/generar-icono.py`.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icon.ico");
        if let Err(e) = resource.compile() {
            // Sin ícono el programa anda igual: no cortar el build.
            println!("cargo:warning=no se pudo agregar el ícono al .exe: {e}");
        }
    }
}

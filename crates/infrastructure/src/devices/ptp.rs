//! Ponte para câmeras PTP que não aparecem como um diretório POSIX.
//!
//! `camera:/`/`gphoto2://` são origens virtuais do desktop. Quando o GVfs está
//! disponível, ele monta a câmera em `/run/user/$UID/gvfs` e o restante do
//! importador trabalha diretamente nesse ponto: a UI lista as fotos e só os
//! arquivos marcados são copiados para a sessão. O download integral via
//! `gphoto2` fica apenas como fallback para ambientes sem GVfs.

use domain::import_source::ImportSource;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Camera {
    nome: String,
    porta: String,
}

/// Detecta câmeras que o libgphoto2 conhece e prefere montá-las pelo GVfs. Se
/// o `gphoto2` ou o GVfs não estiverem instalados, o caminho normal de cartões
/// montados continua funcionando sem erro; em último caso, usa o cache legado.
pub fn fontes_ptp() -> Vec<ImportSource> {
    // Este é o caminho usado pelo Yazi/GVfs e pelo COSMIC Files: uma câmera
    // montada é uma pasta normal para o scanner. Assim não baixamos o cartão
    // inteiro antes de o fotógrafo escolher as fotos.
    let montadas = fontes_gvfs();
    if !montadas.is_empty() {
        return montadas;
    }

    let Some(cameras) = detectar() else {
        return Vec::new();
    };

    // Em desktops Linux o GVfs costuma fazer isso sozinho ao conectar a
    // câmera. Se a sessão não tiver automount, pedimos a montagem uma vez e
    // voltamos a ler o diretório virtual — sem transferir nenhum arquivo.
    montar_com_gvfs(&cameras);
    let montadas = fontes_gvfs();
    if !montadas.is_empty() {
        return montadas;
    }

    cameras
        .into_iter()
        .filter_map(|camera| materializar(camera).ok())
        .collect()
}

/// Procura pontos de montagem PTP criados pelo GVfs. O diretório é o contrato
/// POSIX que o GVfs oferece aos programas que não usam a API GIO diretamente.
fn fontes_gvfs() -> Vec<ImportSource> {
    let Some(uid) = uid_da_sessao() else {
        return Vec::new();
    };
    let raiz = PathBuf::from("/run/user").join(uid).join("gvfs");
    let Ok(entradas) = fs::read_dir(raiz) else {
        return Vec::new();
    };

    entradas
        .filter_map(Result::ok)
        .filter_map(|entrada| {
            let caminho = entrada.path();
            let nome = entrada.file_name().to_string_lossy().to_string();
            // GPhoto2 é a origem PTP do GVfs. MTP também é uma câmera possível,
            // mas fica fora deste adaptador para não confundir celulares com
            // câmeras na lista de cartões.
            nome.starts_with("gphoto2:")
                .then(|| ImportSource::new_device(nome_da_montagem(&nome), caminho))
        })
        .collect()
}

fn uid_da_sessao() -> Option<String> {
    std::env::var("UID").ok().or_else(|| {
        Command::new("id")
            .arg("-u")
            .output()
            .ok()
            .filter(|saida| saida.status.success())
            .map(|saida| String::from_utf8_lossy(&saida.stdout).trim().to_string())
            .filter(|uid| !uid.is_empty())
    })
}

fn montar_com_gvfs(cameras: &[Camera]) {
    if !Command::new("gio").arg("--version").status().is_ok() {
        return;
    }
    for camera in cameras {
        let uri = format!("gphoto2://[{}]/", camera.porta);
        let _ = Command::new("gio").args(["mount", "-d", &uri]).status();
    }
}

fn nome_da_montagem(montagem: &str) -> String {
    montagem
        .split("host=")
        .nth(1)
        .and_then(|host| host.split(['/', ',']).next())
        .filter(|nome| !nome.is_empty())
        .map(|nome| format!("Câmera PTP ({nome})"))
        .unwrap_or_else(|| "Câmera PTP".to_string())
}

fn detectar() -> Option<Vec<Camera>> {
    let saida = Command::new("gphoto2").arg("--auto-detect").output().ok()?;
    if !saida.status.success() {
        return None;
    }
    Some(parsear_deteccao(&String::from_utf8_lossy(&saida.stdout)))
}

fn parsear_deteccao(texto: &str) -> Vec<Camera> {
    texto
        .lines()
        .filter_map(|linha| {
            let linha = linha.trim();
            if linha.is_empty() || linha.starts_with("Model") || linha.starts_with('-') {
                return None;
            }
            let mut partes = linha.split_whitespace();
            let porta = partes.next_back()?.to_string();
            let nome = partes.collect::<Vec<_>>().join(" ");
            (!nome.is_empty()).then_some(Camera { nome, porta })
        })
        .collect()
}

fn materializar(camera: Camera) -> Result<ImportSource, String> {
    let raiz = diretorio_de_cache(&camera.porta)?;
    fs::create_dir_all(&raiz).map_err(|erro| format!("criar cache PTP: {erro}"))?;

    // O gphoto2 grava os arquivos baixados no diretório de trabalho. `--recurse`
    // percorre os diretórios da câmera; `--force-overwrite` torna a atualização
    // da lista idempotente quando a UI consulta as origens novamente.
    let status = Command::new("gphoto2")
        .args([
            "--port",
            &camera.porta,
            "--get-all-files",
            "--recurse",
            "--force-overwrite",
        ])
        .current_dir(&raiz)
        .status()
        .map_err(|erro| format!("executar gphoto2: {erro}"))?;
    if !status.success() {
        return Err(format!("gphoto2 não conseguiu ler {}", camera.nome));
    }

    Ok(ImportSource::new_device(
        format!("{} (PTP)", camera.nome),
        raiz,
    ))
}

fn diretorio_de_cache(porta: &str) -> Result<PathBuf, String> {
    let base = directories::ProjectDirs::from("com", "RecordarFotos", "VintageLightbox")
        .map(|p| p.cache_dir().join("ptp"))
        .or_else(|| {
            std::env::var_os("XDG_CACHE_HOME").map(|p| PathBuf::from(p).join("vintagelightbox/ptp"))
        })
        .ok_or_else(|| "não foi possível encontrar uma pasta de cache".to_string())?;
    let nome = porta
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>();
    Ok(base.join(if nome.is_empty() { "camera" } else { &nome }))
}

/// True quando o seletor do desktop devolveu uma origem virtual, como
/// `camera:/` ou `gphoto2://...`, em vez de um caminho local.
pub fn e_uri_de_camera(caminho: &str) -> bool {
    caminho.starts_with("camera:") || caminho.starts_with("gphoto2:")
}

/// Copia uma origem virtual GVfs/KIO para uma pasta local. É usada quando o
/// usuário escolhe a câmera pelo seletor do sistema em vez do menu de origens.
pub fn materializar_uri(caminho: &str) -> Result<PathBuf, String> {
    let raiz = diretorio_de_cache(caminho)?;
    fs::create_dir_all(&raiz).map_err(|erro| format!("criar cache da câmera: {erro}"))?;
    let destino = raiz.to_string_lossy().to_string();
    let status = Command::new("gio")
        .args(["copy", "-r", caminho, &destino])
        .status()
        .map_err(|erro| format!("executar gio para {caminho}: {erro}"))?;
    if !status.success() {
        return Err(format!("o desktop não conseguiu ler a câmera {caminho}"));
    }
    Ok(raiz)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn parseia_modelo_e_porta_do_auto_detect() {
        let cameras = parsear_deteccao(
            "Model                          Port\n----------------------------------------------------------\nCanon EOS R6                   usb:001,004\n",
        );
        assert_eq!(
            cameras,
            vec![Camera {
                nome: "Canon EOS R6".into(),
                porta: "usb:001,004".into(),
            }]
        );
    }

    #[test]
    fn reconhece_uris_de_camera() {
        assert!(e_uri_de_camera("camera:/"));
        assert!(e_uri_de_camera("gphoto2://[usb:001,004]/"));
        assert!(!e_uri_de_camera("/run/media/alex/CAMERA"));
    }

    #[test]
    fn saneia_a_porta_para_o_cache() {
        let caminho = diretorio_de_cache("usb:001,004").unwrap();
        assert!(caminho.ends_with("usb_001_004"));
    }
}

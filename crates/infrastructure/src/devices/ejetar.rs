//! ⏏️ Ejetar o cartão de onde as fotos vieram.
//!
//! 🚨 **No Linux, cartão arrancado sem ejetar quebra a importação seguinte**
//! (dono, 24/out/2026). Uma câmera PTP montada pelo GVfs que some sem
//! `gio mount -u` deixa a montagem em `/run/user/$UID/gvfs` pendurada: a lista
//! de cartões continua mostrando a câmera que já saiu, e a próxima leitura trava
//! nela. Um cartão de disco (`/run/media/…`) arrancado montado deixa o udisks
//! com o ponto de montagem velho. Por isso o app ejeta sozinho ao fim de cada
//! importação, e diz ao operador que já pode tirar.
//!
//! 🔑 **Ejetou é "a montagem sumiu"**, e não "o comando devolveu 0". O verbo
//! "Ejetar" do Explorer não devolve status nenhum, e o `gio` às vezes sai 0 com
//! a montagem ainda de pé. Quem decide é a lista de cartões relida depois.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use sysinfo::Disks;

use super::{e_de_fora, nome_do_cartao, ptp};

/// Um cartão (ou câmera) montado, com o que é preciso para ejetá-lo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cartao {
    /// O rótulo que o operador vê — o mesmo da lista "Cartões".
    pub nome: String,
    /// Onde ele está montado.
    pub raiz: PathBuf,
    tipo: Tipo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tipo {
    /// Um disco: cartão no leitor, pendrive, câmera em modo armazenamento.
    Disco {
        /// O dispositivo de bloco (`/dev/sdb1`) no Linux; o nome do volume no
        /// macOS e no Windows.
        dispositivo: String,
    },
    /// Uma câmera PTP que o GVfs expõe como pasta.
    CameraGvfs {
        /// `gphoto2://<host>/` — o endereço que o `gio` desmonta.
        uri: String,
    },
}

/// Os cartões montados agora.
pub fn cartoes_montados() -> Vec<Cartao> {
    let disks = Disks::new_with_refreshed_list();
    let mut cartoes: Vec<Cartao> = disks
        .iter()
        .filter(|disco| e_de_fora(disco.mount_point(), disco.is_removable()))
        .map(|disco| Cartao {
            nome: nome_do_cartao(disco.name().to_str(), disco.mount_point()),
            raiz: disco.mount_point().to_path_buf(),
            tipo: Tipo::Disco {
                dispositivo: disco.name().to_string_lossy().into_owned(),
            },
        })
        .collect();

    cartoes.extend(
        ptp::montagens_gvfs()
            .into_iter()
            .filter_map(|(nome, raiz)| {
                let pasta = raiz.file_name()?.to_str()?.to_string();
                Some(Cartao {
                    nome,
                    tipo: Tipo::CameraGvfs {
                        uri: uri_do_gvfs(&pasta)?,
                    },
                    raiz,
                })
            }),
    );
    cartoes
}

/// O cartão onde `arquivo` está — `None` quando ele está no disco do sistema.
pub fn cartao_de(arquivo: &Path) -> Option<Cartao> {
    escolher(cartoes_montados(), arquivo)
}

/// 🔑 **A montagem mais funda que contém o arquivo.** Um cartão montado em
/// `/Volumes/A` não pode ganhar de `/Volumes/A B`; `Path::starts_with` compara
/// por componente, e o mais longo vence quando um está dentro do outro.
fn escolher(cartoes: Vec<Cartao>, arquivo: &Path) -> Option<Cartao> {
    cartoes
        .into_iter()
        .filter(|cartao| cartao.raiz.parent().is_some() && arquivo.starts_with(&cartao.raiz))
        .max_by_key(|cartao| cartao.raiz.components().count())
}

/// Ejeta o cartão e confere que ele saiu da lista.
///
/// ⚠️ **Cartão ocupado é comum logo depois da cópia**: a miniatura da última
/// foto ainda lendo, o Finder indexando. Por isso três tentativas com uma
/// pausa entre elas antes de dizer que não deu. Bloqueante — quem chama roda
/// numa thread de fundo.
pub fn ejetar(cartao: &Cartao) -> Result<(), String> {
    let mut ultimo_erro = String::new();
    for tentativa in 0..3 {
        if tentativa > 0 {
            std::thread::sleep(Duration::from_secs(2));
        }
        // 🔑 Recusado pelo sistema, olha uma vez e segue para a próxima
        // tentativa; aceito, dá até 5 s para a montagem sair da lista.
        let paciencia = match pedir_ejecao(cartao) {
            Ok(()) => 10,
            Err(erro) => {
                ultimo_erro = erro;
                1
            }
        };
        if esperar_sumir(&cartao.raiz, paciencia) {
            return Ok(());
        }
        if ultimo_erro.is_empty() {
            ultimo_erro = "o sistema não soltou o cartão (alguém ainda está lendo dele)".into();
        }
    }
    Err(ultimo_erro)
}

/// Olha `vezes` vezes, meio segundo entre elas, se a montagem saiu da lista.
fn esperar_sumir(raiz: &Path, vezes: usize) -> bool {
    for vez in 0..vezes {
        if vez > 0 {
            std::thread::sleep(Duration::from_millis(500));
        }
        if !cartoes_montados().iter().any(|c| c.raiz == raiz) {
            return true;
        }
    }
    false
}

fn pedir_ejecao(cartao: &Cartao) -> Result<(), String> {
    match &cartao.tipo {
        Tipo::CameraGvfs { uri } => rodar("gio", &["mount", "-u", uri]),
        Tipo::Disco { dispositivo } => ejetar_disco(&cartao.raiz, dispositivo),
    }
}

#[cfg(target_os = "macos")]
fn ejetar_disco(raiz: &Path, _dispositivo: &str) -> Result<(), String> {
    // `eject`, e não `unmount`: solta o disco inteiro, e o Finder para de
    // mostrar o cartão — é o mesmo do botão ⏏ da barra lateral.
    rodar("diskutil", &["eject", &raiz.to_string_lossy()])
}

#[cfg(windows)]
fn ejetar_disco(raiz: &Path, _dispositivo: &str) -> Result<(), String> {
    // O verbo "Eject" do Explorer — o mesmo do menu de botão direito. O nome
    // é o canônico, e funciona no Windows em português.
    let unidade = raiz
        .to_string_lossy()
        .trim_end_matches(['\\', '/'])
        .replace('\'', "''");
    let script = format!(
        "(New-Object -ComObject Shell.Application).Namespace(17).ParseName('{unidade}')\
         .InvokeVerb('Eject')"
    );
    rodar(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", &script],
    )
}

#[cfg(all(unix, not(target_os = "macos")))]
fn ejetar_disco(raiz: &Path, dispositivo: &str) -> Result<(), String> {
    // 🔑 `gio mount -e` primeiro: desmonta **e** desliga, como o ⏏ do
    // Nautilus, e não pede senha numa sessão de desktop.
    let raiz = raiz.to_string_lossy();
    let pelo_gio = rodar("gio", &["mount", "-e", &raiz]);
    if pelo_gio.is_ok() || !dispositivo.starts_with("/dev/") {
        return pelo_gio;
    }
    // Sem GVfs: o udisks direto. O `power-off` falha em leitor interno
    // (`mmcblk`), e tudo bem — desmontado já pode sair.
    rodar("udisksctl", &["unmount", "-b", dispositivo])?;
    let _ = rodar("udisksctl", &["power-off", "-b", dispositivo]);
    Ok(())
}

fn rodar(programa: &str, argumentos: &[&str]) -> Result<(), String> {
    let saida = Command::new(programa)
        .args(argumentos)
        .output()
        .map_err(|erro| format!("{programa} não rodou: {erro}"))?;
    if saida.status.success() {
        return Ok(());
    }
    let motivo = String::from_utf8_lossy(&saida.stderr).trim().to_string();
    Err(if motivo.is_empty() {
        format!("{programa} recusou")
    } else {
        motivo
    })
}

/// A pasta que o GVfs cria em `/run/user/$UID/gvfs` vira o endereço que o
/// `gio` entende: `gphoto2:host=X` é `gphoto2://X/`.
fn uri_do_gvfs(pasta: &str) -> Option<String> {
    let host = pasta.strip_prefix("gphoto2:host=")?;
    let host = host.split(',').next().filter(|h| !h.is_empty())?;
    Some(format!("gphoto2://{host}/"))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn disco(raiz: &str) -> Cartao {
        Cartao {
            nome: raiz.rsplit('/').next().unwrap_or_default().into(),
            raiz: PathBuf::from(raiz),
            tipo: Tipo::Disco {
                dispositivo: "/dev/sdb1".into(),
            },
        }
    }

    #[test]
    fn o_arquivo_do_cartao_acha_o_cartao() {
        let achado = escolher(
            vec![disco("/run/media/alex/NIKON D3100")],
            Path::new("/run/media/alex/NIKON D3100/DCIM/100NIKON/DSC_0001.NEF"),
        );
        assert_eq!(achado.map(|c| c.nome), Some("NIKON D3100".into()));
    }

    /// 🚨 `/Volumes/A` não é dono de `/Volumes/A B/…`: a comparação é por
    /// componente, e não por texto.
    #[test]
    fn nome_parecido_nao_confunde_o_cartao() {
        let achado = escolher(
            vec![disco("/Volumes/EOS"), disco("/Volumes/EOS 2")],
            Path::new("/Volumes/EOS 2/DCIM/IMG_1.CR3"),
        );
        assert_eq!(
            achado.map(|c| c.raiz),
            Some(PathBuf::from("/Volumes/EOS 2"))
        );
    }

    #[test]
    fn a_montagem_mais_funda_vence() {
        let achado = escolher(
            vec![disco("/mnt"), disco("/mnt/cartao")],
            Path::new("/mnt/cartao/DCIM/a.jpg"),
        );
        assert_eq!(achado.map(|c| c.raiz), Some(PathBuf::from("/mnt/cartao")));
    }

    /// Foto de pasta do disco não ejeta nada — nem a raiz montada solta.
    #[test]
    fn foto_de_pasta_nao_tem_cartao() {
        assert_eq!(
            escolher(
                vec![disco("/Volumes/EOS"), disco("/")],
                Path::new("/Users/alex/Fotos/a.jpg")
            ),
            None
        );
    }

    #[test]
    fn a_pasta_do_gvfs_vira_o_endereco_do_gio() {
        assert_eq!(
            uri_do_gvfs("gphoto2:host=Canon_Inc._Canon_Digital_Camera"),
            Some("gphoto2://Canon_Inc._Canon_Digital_Camera/".into())
        );
        assert_eq!(
            uri_do_gvfs("gphoto2:host=%5Busb%3A001%2C004%5D"),
            Some("gphoto2://%5Busb%3A001%2C004%5D/".into())
        );
        assert_eq!(uri_do_gvfs("mtp:host=Android"), None);
    }
}

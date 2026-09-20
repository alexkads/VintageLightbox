pub mod history_repo;
pub mod repository;
use domain::import_source::ImportSource;
use std::path::Path;
use sysinfo::Disks;

pub struct DeviceService;

impl Default for DeviceService {
    fn default() -> Self {
        Self::new()
    }
}

/// Onde cada sistema monta o que se pluga.
///
/// 🚨 **O Linux não estava aqui, e o cartão sumia da lista.** Até 19/set/2026 o
/// filtro era `is_removable() || /Volumes/` — o `/Volumes/` do macOS escrito à
/// mão, e nada para os outros dois. Um leitor de cartão interno (`mmcblk`) ou um
/// pendrive atrás de um hub costumam chegar com `removable = 0` no
/// `/sys/block/*/removable`, e aí o cartão não aparecia em "Cartões" **nem**
/// quando estava montado e visível no Nautilus.
const RAIZES_DE_MONTAGEM: [&str; 4] = [
    // macOS
    "/Volumes/",
    // Linux: udisks2 ≥ 2.9 (Ubuntu 25.x) usa /run/media/$USER/…
    "/run/media/",
    // Linux: udisks2 antigo, e o que ainda monta à mão
    "/media/",
    "/mnt/",
];

impl DeviceService {
    pub fn new() -> Self {
        Self
    }

    pub fn get_mounted_devices(&self) -> Vec<ImportSource> {
        let disks = Disks::new_with_refreshed_list();

        disks
            .iter()
            .filter_map(|disco| {
                let ponto = disco.mount_point();
                e_de_fora(ponto, disco.is_removable()).then(|| {
                    ImportSource::new_device(
                        nome_do_cartao(disco.name().to_str(), ponto),
                        ponto.to_path_buf(),
                    )
                })
            })
            .collect()
    }
}

/// Se este disco é algo que alguém plugou — e não o sistema.
fn e_de_fora(ponto: &Path, removivel: bool) -> bool {
    let texto = ponto.to_string_lossy();
    removivel || RAIZES_DE_MONTAGEM.iter().any(|raiz| texto.starts_with(raiz))
}

/// O nome que vai no botão de "Cartões".
///
/// 🚨 **`disk.name()` não é o nome do cartão no Linux** — é o dispositivo de
/// bloco. Um cartão Nikon montado em `/run/media/alex/NIKON D3100` chegava aqui
/// como `"/dev/sdb1"`, e era isso que o botão mostrava: a lista de origens do
/// modal de importação ficava com um caminho de `/dev` no lugar do rótulo do
/// cartão. No macOS o mesmo `name()` devolve o nome do volume, e por isso o
/// defeito atravessou intacto — quem escreveu a fallback só previu o nome
/// **vazio**.
///
/// A pasta de montagem é o rótulo em todos os três sistemas: é ela que o Finder,
/// o Nautilus e o Explorer mostram.
fn nome_do_cartao(nome: Option<&str>, ponto: &Path) -> String {
    let da_pasta = || {
        ponto
            .file_name()
            .and_then(|f| f.to_str())
            .map(str::to_string)
    };

    match nome.map(str::trim) {
        // Um caminho de dispositivo não é nome de nada que se mostre.
        Some(n) if !n.is_empty() && !n.starts_with('/') => n.to_string(),
        _ => da_pasta().unwrap_or_else(|| "Dispositivo".to_string()),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// 🚨 O caso medido no Ubuntu 25.10 em 19/set/2026: `sysinfo` devolve
    /// `name = "/dev/sdb1"` para o cartão montado em
    /// `/run/media/alex-silva/NIKON D3100`.
    #[test]
    fn no_linux_o_nome_vem_da_pasta_e_nao_do_dev() {
        let nome = nome_do_cartao(
            Some("/dev/sdb1"),
            Path::new("/run/media/alex-silva/NIKON D3100"),
        );

        assert_eq!(nome, "NIKON D3100");
    }

    /// No macOS o `name()` já é o nome do volume, e ele vence.
    #[test]
    fn no_mac_o_nome_do_volume_e_usado() {
        let nome = nome_do_cartao(Some("NIKON D3100"), Path::new("/Volumes/NIKON D3100"));

        assert_eq!(nome, "NIKON D3100");
    }

    #[test]
    fn nome_vazio_cai_na_pasta() {
        let nome = nome_do_cartao(Some("  "), Path::new("/media/alex/EOS_DIGITAL"));

        assert_eq!(nome, "EOS_DIGITAL");
    }

    /// Sem nome e sem pasta — a raiz de um sistema de arquivos montada solta.
    #[test]
    fn sem_nome_e_sem_pasta_sobra_a_palavra() {
        assert_eq!(nome_do_cartao(None, Path::new("/")), "Dispositivo");
    }

    /// 🚨 O cartão que o kernel não marca como removível continua sendo cartão.
    #[test]
    fn montado_em_run_media_conta_mesmo_sem_a_marca_de_removivel() {
        assert!(e_de_fora(
            Path::new("/run/media/alex-silva/NIKON D3100"),
            false
        ));
        assert!(e_de_fora(Path::new("/media/alex/EOS_DIGITAL"), false));
        assert!(e_de_fora(Path::new("/Volumes/Cartao"), false));
    }

    /// E o disco do sistema continua fora da lista.
    #[test]
    fn o_disco_do_sistema_nao_e_cartao() {
        assert!(!e_de_fora(Path::new("/"), false));
        assert!(!e_de_fora(Path::new("/boot/efi"), false));
        assert!(!e_de_fora(Path::new("/home"), false));
    }

    /// A marca de removível sozinha basta — um pendrive montado em qualquer
    /// lugar continua aparecendo.
    #[test]
    fn removivel_entra_de_qualquer_ponto() {
        assert!(e_de_fora(Path::new("/qualquer/lugar"), true));
    }
}

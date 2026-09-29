//! Os cartões plugados, vistos como dispositivo — para a recuperação.
//!
//! O [`super::DeviceService`] lista **volumes montados**, e é o que a importação
//! precisa. A recuperação precisa do dispositivo por baixo: `/dev/mmcblk0`,
//! `/dev/rdisk4`, `\\.\PhysicalDrive2`. Cada sistema tem o seu jeito de dizer
//! quais são, e cada jeito vira aqui uma função pura sobre o texto que o sistema
//! devolve, testada com a saída gravada. O disco do sistema é filtrado duas
//! vezes: aqui (só o removível entra) e no `use_cases::recuperacao` (nada com
//! `/` ou `C:\` montado).

use domain::recuperacao::{CartaoBruto, CartoesBrutos};

#[derive(Default)]
pub struct CartoesDoSistema;

impl CartoesBrutos for CartoesDoSistema {
    fn listar(&self) -> Vec<CartaoBruto> {
        #[cfg(target_os = "linux")]
        {
            linux::listar()
        }
        #[cfg(target_os = "macos")]
        {
            macos::listar()
        }
        #[cfg(target_os = "windows")]
        {
            windows::listar()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Vec::new()
        }
    }

    fn raizes_do_sistema(&self) -> Vec<String> {
        #[cfg(target_os = "windows")]
        {
            let unidade = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
            vec![format!("{unidade}\\")]
        }
        #[cfg(target_os = "macos")]
        {
            vec!["/".into(), "/System/Volumes/Data".into()]
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            ["/", "/boot", "/boot/efi", "/usr", "/var", "/home"]
                .map(String::from)
                .to_vec()
        }
    }
}

/// Os montados de um dispositivo, a partir do `/proc/mounts` (Linux) ou da
/// saída do `mount` (macOS): toda linha cuja primeira coluna é o dispositivo ou
/// uma partição dele.
pub fn montagens_de(dispositivo: &str, mounts: &str) -> Vec<String> {
    mounts
        .lines()
        .filter_map(|linha| {
            let mut partes = linha.split_whitespace();
            let origem = partes.next()?;
            let mut destino = partes.next()?;
            if destino == "on" {
                // macOS: `/dev/disk4s1 on /Volumes/EOS_DIGITAL (msdos, …)`.
                // O ponto de montagem pode ter espaço; vai até o " (".
                let depois = linha.split_once(" on ")?.1;
                destino = depois.rsplit_once(" (").map_or(depois, |(d, _)| d);
            }
            e_particao_de(origem, dispositivo).then(|| desescapar(destino))
        })
        .collect()
}

/// `/dev/sdb1` é partição de `/dev/sdb`; `/dev/sdbc` não. O `mmcblk0p1` e o
/// `disk4s1` trazem uma letra antes do número.
fn e_particao_de(origem: &str, dispositivo: &str) -> bool {
    let Some(resto) = origem.strip_prefix(dispositivo) else {
        return false;
    };
    let resto = resto
        .strip_prefix('p')
        .or_else(|| resto.strip_prefix('s'))
        .unwrap_or(resto);
    resto.chars().all(|c| c.is_ascii_digit())
}

/// O `/proc/mounts` escreve o espaço como `\040`.
fn desescapar(caminho: &str) -> String {
    let mut saida = String::new();
    let mut chars = caminho.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            let octal: String = chars.clone().take(3).collect();
            if octal.len() == 3 && octal.chars().all(|d| ('0'..='7').contains(&d)) {
                if let Ok(v) = u8::from_str_radix(&octal, 8) {
                    saida.push(v as char);
                    for _ in 0..3 {
                        chars.next();
                    }
                    continue;
                }
            }
        }
        saida.push(c);
    }
    saida
}

/// O que o `/sys/block/<nome>` diz de um dispositivo.
#[derive(Debug, Clone, Default)]
pub struct DoSysBlock {
    pub nome: String,
    pub removivel: bool,
    /// Se o caminho real do dispositivo passa por `/usb`: o leitor USB que se
    /// diz fixo.
    pub usb: bool,
    pub setores: u64,
    pub modelo: String,
}

/// Quais entradas do `/sys/block` são cartão, e como aparecem.
pub fn cartoes_do_linux(entradas: Vec<DoSysBlock>, mounts: &str) -> Vec<CartaoBruto> {
    entradas
        .into_iter()
        .filter(|e| {
            let virtual_ = ["loop", "ram", "zram", "dm-", "md", "sr", "nbd", "nvme"]
                .iter()
                .any(|p| e.nome.starts_with(p));
            let mmc = e.nome.starts_with("mmcblk")
                && !e.nome.contains("boot")
                && !e.nome.contains("rpmb");
            !virtual_ && (mmc || e.removivel || e.usb)
        })
        .map(|e| {
            let dispositivo = format!("/dev/{}", e.nome);
            let montagens = montagens_de(&dispositivo, mounts);
            let nome = nome_do_cartao(&e.modelo, &montagens, &e.nome);
            CartaoBruto {
                dispositivo,
                nome,
                tamanho: e.setores * 512,
                montagens,
            }
        })
        .collect()
}

/// O nome do volume quando há um (`EOS_DIGITAL`), senão o modelo do leitor.
fn nome_do_cartao(modelo: &str, montagens: &[String], reserva: &str) -> String {
    let volume = montagens.iter().find_map(|m| {
        let nome = m.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next()?;
        (!nome.is_empty() && !nome.ends_with(':')).then(|| nome.to_string())
    });
    let modelo = modelo.trim();
    match (volume, modelo.is_empty()) {
        (Some(v), true) => v,
        (Some(v), false) => format!("{v} ({modelo})"),
        (None, false) => modelo.to_string(),
        (None, true) => reserva.to_string(),
    }
}

/// Os discos da saída de `diskutil list external physical`.
pub fn discos_do_diskutil(lista: &str) -> Vec<String> {
    lista
        .lines()
        .filter_map(|l| {
            let disco = l.strip_prefix("/dev/")?.split_whitespace().next()?;
            disco.starts_with("disk").then(|| disco.to_string())
        })
        .collect()
}

/// O tamanho exato em bytes, da saída de `diskutil info`:
/// `Disk Size: 31.9 GB (31914983424 Bytes) (exactly …)`.
pub fn tamanho_do_diskutil(info: &str) -> u64 {
    info.lines()
        .find(|l| l.trim_start().starts_with("Disk Size:"))
        .and_then(|l| {
            let (antes, _) = l.split_once(" Bytes)")?;
            antes.rsplit('(').next()?.trim().parse().ok()
        })
        .unwrap_or(0)
}

/// O modelo, da saída de `diskutil info`: `Device / Media Name: SD Card Reader`.
pub fn modelo_do_diskutil(info: &str) -> String {
    info.lines()
        .find_map(|l| l.trim_start().strip_prefix("Device / Media Name:"))
        .map(|m| m.trim().to_string())
        .unwrap_or_default()
}

/// Os discos da consulta do PowerShell, uma linha cada:
/// `índice|bytes|modelo|interface|tipo de mídia|E:\,F:\`.
pub fn cartoes_do_windows(saida: &str) -> Vec<CartaoBruto> {
    saida
        .lines()
        .filter_map(|l| {
            let c: Vec<&str> = l.trim().split('|').collect();
            if c.len() < 6 {
                return None;
            }
            let indice: u32 = c[0].trim().parse().ok()?;
            let tamanho: u64 = c[1].trim().parse().unwrap_or(0);
            let (modelo, interface, midia) = (c[2].trim(), c[3].trim(), c[4].trim());
            // ⚠️ Nada de procurar "SD" no modelo: "Samsung SSD" também tem.
            let removivel = midia.contains("Removable") || interface.eq_ignore_ascii_case("USB");
            if !removivel {
                return None;
            }
            let montagens: Vec<String> = c[5]
                .split(',')
                .map(str::trim)
                .filter(|m| !m.is_empty())
                .map(String::from)
                .collect();
            let nome = match montagens.first() {
                Some(letra) => format!("{modelo} ({letra})"),
                None => modelo.to_string(),
            };
            Some(CartaoBruto {
                dispositivo: format!("\\\\.\\PhysicalDrive{indice}"),
                nome,
                tamanho,
                montagens,
            })
        })
        .collect()
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::fs;

    pub fn listar() -> Vec<CartaoBruto> {
        let Ok(dir) = fs::read_dir("/sys/block") else {
            return Vec::new();
        };
        let ler = |caminho: std::path::PathBuf| {
            fs::read_to_string(caminho)
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        };
        let entradas = dir
            .flatten()
            .map(|e| {
                let nome = e.file_name().to_string_lossy().to_string();
                let base = e.path();
                let modelo = {
                    let m = ler(base.join("device/model"));
                    if m.is_empty() {
                        ler(base.join("device/name"))
                    } else {
                        m
                    }
                };
                DoSysBlock {
                    removivel: ler(base.join("removable")) == "1",
                    usb: fs::canonicalize(&base)
                        .map(|p| p.to_string_lossy().contains("/usb"))
                        .unwrap_or(false),
                    setores: ler(base.join("size")).parse().unwrap_or(0),
                    modelo,
                    nome,
                }
            })
            .collect();
        let mounts = fs::read_to_string("/proc/mounts").unwrap_or_default();
        cartoes_do_linux(entradas, &mounts)
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::process::Command;

    fn rodar(programa: &str, args: &[&str]) -> String {
        Command::new(programa)
            .args(args)
            .output()
            .map(|s| String::from_utf8_lossy(&s.stdout).to_string())
            .unwrap_or_default()
    }

    pub fn listar() -> Vec<CartaoBruto> {
        let mounts = rodar("mount", &[]);
        discos_do_diskutil(&rodar("diskutil", &["list", "external", "physical"]))
            .into_iter()
            .map(|disco| {
                let info = rodar("diskutil", &["info", &disco]);
                let montagens = montagens_de(&format!("/dev/{disco}"), &mounts);
                let nome = nome_do_cartao(&modelo_do_diskutil(&info), &montagens, &disco);
                CartaoBruto {
                    // 🔑 O `rdisk` é o mesmo disco sem o cache do sistema: lê
                    // várias vezes mais rápido, e é o que se varre.
                    dispositivo: format!("/dev/r{disco}"),
                    nome,
                    tamanho: tamanho_do_diskutil(&info),
                    montagens,
                }
            })
            .collect()
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    /// Uma linha por disco, com as letras montadas nele. `Win32_DiskDrive`, e
    /// não `Get-Disk`: este pede administrador, e a lista abre antes da
    /// elevação.
    const CONSULTA: &str = "Get-CimInstance Win32_DiskDrive | ForEach-Object { \
        $d = $_; \
        $l = (Get-CimAssociatedInstance -InputObject $d -ResultClassName Win32_DiskPartition | \
              ForEach-Object { Get-CimAssociatedInstance -InputObject $_ -ResultClassName Win32_LogicalDisk } | \
              ForEach-Object { $_.DeviceID + '\\' }) -join ','; \
        \"$($d.Index)|$($d.Size)|$($d.Model)|$($d.InterfaceType)|$($d.MediaType)|$l\" }";

    pub fn listar() -> Vec<CartaoBruto> {
        let saida = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", CONSULTA])
            // CREATE_NO_WINDOW: o PowerShell não pisca uma janela preta no balcão.
            .creation_flags(0x0800_0000)
            .output()
            .map(|s| String::from_utf8_lossy(&s.stdout).to_string())
            .unwrap_or_default();
        cartoes_do_windows(&saida)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const PROC_MOUNTS: &str = "\
/dev/nvme0n1p2 / ext4 rw,relatime 0 0
/dev/nvme0n1p1 /boot/efi vfat rw 0 0
/dev/mmcblk0p1 /media/ana/EOS_DIGITAL vfat rw,nosuid 0 0
/dev/sdb1 /run/media/ana/NIKON\\040D750 exfat rw 0 0
/dev/sdbc1 /mnt/outro ext4 rw 0 0
";

    fn sys(nome: &str, removivel: bool, usb: bool, setores: u64, modelo: &str) -> DoSysBlock {
        DoSysBlock {
            nome: nome.into(),
            removivel,
            usb,
            setores,
            modelo: modelo.into(),
        }
    }

    #[test]
    fn no_linux_so_o_removivel_entra() {
        let cartoes = cartoes_do_linux(
            vec![
                sys("nvme0n1", false, false, 1_000_000_000, "Samsung SSD"),
                sys("loop0", false, false, 100_000, ""),
                sys("mmcblk0", false, false, 62_333_952, "SD64G"),
                sys("mmcblk0boot0", false, false, 8192, ""),
                sys("sdb", false, true, 124_735_488, "Card Reader"),
                sys("sda", false, false, 900_000_000, "HD interno"),
            ],
            PROC_MOUNTS,
        );
        let nomes: Vec<_> = cartoes.iter().map(|c| c.dispositivo.as_str()).collect();
        assert_eq!(nomes, ["/dev/mmcblk0", "/dev/sdb"]);
        assert_eq!(cartoes[0].tamanho, 62_333_952 * 512);
        assert_eq!(cartoes[0].montagens, ["/media/ana/EOS_DIGITAL"]);
        assert_eq!(cartoes[0].nome, "EOS_DIGITAL (SD64G)");
        // O espaço escapado volta a ser espaço, e `sdbc1` não é de `sdb`.
        assert_eq!(cartoes[1].montagens, ["/run/media/ana/NIKON D750"]);
    }

    #[test]
    fn no_macos_le_disco_tamanho_e_montagem() {
        let lista = "\
/dev/disk4 (external, physical):
   #:                       TYPE NAME                    SIZE       IDENTIFIER
   0:     FDisk_partition_scheme                        *31.9 GB    disk4
   1:                 DOS_FAT_32 EOS_DIGITAL             31.9 GB    disk4s1
";
        assert_eq!(discos_do_diskutil(lista), ["disk4"]);
        let info = "   Device / Media Name:       SD Card Reader\n   Disk Size:                 31.9 GB (31914983424 Bytes) (exactly 62333952 512-Byte-Units)\n";
        assert_eq!(tamanho_do_diskutil(info), 31_914_983_424);
        assert_eq!(modelo_do_diskutil(info), "SD Card Reader");
        let mount = "/dev/disk1s1 on / (apfs, local, journaled)\n/dev/disk4s1 on /Volumes/EOS DIGITAL (msdos, local, nodev, nosuid, noowners)\n";
        assert_eq!(montagens_de("/dev/disk4", mount), ["/Volumes/EOS DIGITAL"]);
    }

    #[test]
    fn no_windows_o_disco_fixo_fica_de_fora() {
        let saida = "\
0|512105932800|Samsung SSD 980|SCSI|Fixed hard disk media|C:\\
2|63864569856|SDXC Card|SCSI|Removable Media|E:\\
3|31914983424|Generic USB Reader|USB|Removable Media|
";
        let cartoes = cartoes_do_windows(saida);
        assert_eq!(cartoes.len(), 2);
        assert_eq!(cartoes[0].dispositivo, "\\\\.\\PhysicalDrive2");
        assert_eq!(cartoes[0].montagens, ["E:\\"]);
        assert_eq!(cartoes[0].nome, "SDXC Card (E:\\)");
        assert!(cartoes[1].montagens.is_empty());
    }
}

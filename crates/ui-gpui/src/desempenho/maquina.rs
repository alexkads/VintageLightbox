//! 🖥️ A máquina da medição: hardware, sistema e drivers.
//!
//! # Por quê (dono, 2026-09-27)
//!
//! *"Nesse relatório traga informações do Hardware, sistema operacional e
//! drivers. Com isso poderemos saber se o problema é na aplicação ou
//! limitações."* Um p95 de 40 ms num notebook de 2015 com vídeo integrado e na
//! bateria não diz o mesmo que num desktop com placa dedicada — e um motor que
//! abriu na integrada (ou no OpenGL) com uma dedicada ao lado é defeito do
//! app, não da máquina.
//!
//! # De onde vem cada coisa
//!
//! | Dado | macOS | Windows | Linux |
//! |---|---|---|---|
//! | CPU, memória, sistema, kernel | `sysinfo` | `sysinfo` | `sysinfo` |
//! | GPUs, backend, driver do wgpu | `wgpu` (todas as que ele enxerga) | idem | idem |
//! | driver do sistema, monitores, Hz | `system_profiler` | `Win32_VideoController` (CIM) | `xrandr`, `/sys`, `/proc` |
//! | energia | `pmset` | `Win32_Battery` + `powercfg` | `/sys/class/power_supply` |
//!
//! 🔑 **Coletada uma vez, numa thread de fundo** ([`coletar_em_fundo`]): as
//! consultas ao sistema levam de centenas de milissegundos a alguns segundos, e
//! enumerar as GPUs abre cada backend. Nada disso roda na thread da interface.
//!
//! 🔒 **Sem identificação**: nem hostname, nem número de série de monitor, nem
//! usuário — só o que descreve a capacidade da máquina.

use std::process::{Command, Stdio};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Gpu {
    pub nome: String,
    pub fabricante: String,
    pub fabricante_id: String,
    pub placa_id: String,
    /// `integrada`, `dedicada`, `virtual`, `cpu`, `outra`.
    pub tipo: String,
    pub backend: String,
    /// O que o wgpu diz do driver (Vulkan e DX12 costumam dizer nome e
    /// versão; Metal não).
    pub driver: String,
    pub driver_info: String,
    pub carimbos: bool,
}

/// A placa como o sistema a descreve — é aqui que mora a versão do driver no
/// Windows.
#[derive(Clone, Debug, Default, Serialize)]
pub struct PlacaNoSistema {
    pub nome: String,
    pub driver_versao: String,
    pub driver_data: String,
    pub memoria_mb: Option<u64>,
    pub detalhe: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Monitor {
    pub nome: String,
    pub resolucao: String,
    pub hz: Option<f32>,
    pub principal: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Maquina {
    pub sistema: String,
    pub sistema_versao: String,
    pub kernel: String,
    pub arquitetura: String,
    pub area_de_trabalho: Option<String>,
    pub cpu: String,
    pub cpu_fabricante: String,
    pub nucleos_fisicos: Option<usize>,
    pub nucleos_logicos: usize,
    pub cpu_mhz: u64,
    pub memoria_total_mb: u64,
    pub memoria_livre_mb: u64,
    pub gpus: Vec<Gpu>,
    pub placas_no_sistema: Vec<PlacaNoSistema>,
    pub monitores: Vec<Monitor>,
    pub energia: Option<String>,
    /// Quem desenha a interface — o renderizador do GPUI neste sistema.
    pub renderizador_da_interface: &'static str,
    pub versao_do_app: &'static str,
    pub perfil_de_build: &'static str,
    /// O que não se conseguiu ler, e por quê.
    pub faltas: Vec<String>,
    pub coleta_ms: u64,
}

impl Maquina {
    /// A taxa do monitor principal, se o sistema disse.
    pub fn hz_do_principal(&self) -> Option<f32> {
        self.monitores
            .iter()
            .find(|m| m.principal)
            .or_else(|| self.monitores.first())
            .and_then(|m| m.hz)
    }

    /// Uma linha para o cabeçalho: "Apple M2 Pro · 12 núcleos · 32 GB".
    pub fn cpu_em_uma_linha(&self) -> String {
        format!(
            "{} · {} núcleos{} · {:.0} GB",
            self.cpu,
            self.nucleos_logicos,
            self.nucleos_fisicos
                .map(|f| format!(" ({f} físicos)"))
                .unwrap_or_default(),
            self.memoria_total_mb as f64 / 1024.0
        )
    }
}

static MAQUINA: OnceLock<Arc<Maquina>> = OnceLock::new();
static COLETANDO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A máquina, se a coleta já terminou.
pub fn pronta() -> Option<Arc<Maquina>> {
    MAQUINA.get().cloned()
}

/// Começa a coleta numa thread de fundo, uma vez por abertura do app.
pub fn coletar_em_fundo() {
    if MAQUINA.get().is_some() || COLETANDO.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("desempenho: máquina".into())
        .spawn(|| {
            let m = coletar();
            let hz = m.hz_do_principal();
            let _ = MAQUINA.set(Arc::new(m));
            if let Some(hz) = hz {
                super::atualizar_contexto(|c| {
                    if c.hz_origem != "sistema" {
                        c.hz = hz;
                        c.hz_origem = "sistema";
                    }
                });
            }
        });
}

/// Espera a coleta por até `prazo` — só de fora da thread da interface (a
/// gravação no banco, que já roda em segundo plano).
pub fn esperar(prazo: Duration) -> Option<Arc<Maquina>> {
    coletar_em_fundo();
    let fim = Instant::now() + prazo;
    while Instant::now() < fim {
        if let Some(m) = pronta() {
            return Some(m);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    pronta()
}

fn renderizador() -> &'static str {
    if cfg!(target_os = "windows") {
        "GPUI · DirectX 11 (DirectComposition)"
    } else if cfg!(target_os = "macos") {
        "GPUI · Metal"
    } else {
        "GPUI · wgpu (Vulkan ou OpenGL)"
    }
}

/// Toda a coleta, síncrona. Ver o [módulo](self).
pub fn coletar() -> Maquina {
    let comeco = Instant::now();
    let mut sys = sysinfo::System::new();
    sys.refresh_cpu_all();
    sys.refresh_memory();
    let cpu = sys.cpus().first();
    let mut m = Maquina {
        sistema: crate::telemetria::maquina::deste().sistema.clone(),
        sistema_versao: sysinfo::System::long_os_version().unwrap_or_default(),
        kernel: sysinfo::System::kernel_version().unwrap_or_default(),
        arquitetura: std::env::consts::ARCH.into(),
        area_de_trabalho: area_de_trabalho(),
        cpu: cpu
            .map(|c| c.brand().trim().to_string())
            .unwrap_or_default(),
        cpu_fabricante: cpu.map(|c| c.vendor_id().to_string()).unwrap_or_default(),
        nucleos_fisicos: sysinfo::System::physical_core_count(),
        nucleos_logicos: sys.cpus().len(),
        cpu_mhz: cpu.map(|c| c.frequency()).unwrap_or(0),
        memoria_total_mb: sys.total_memory() / (1024 * 1024),
        memoria_livre_mb: sys.available_memory() / (1024 * 1024),
        renderizador_da_interface: renderizador(),
        versao_do_app: env!("CARGO_PKG_VERSION"),
        perfil_de_build: super::perfil_de_build(),
        ..Default::default()
    };
    m.gpus = revelacao_core::adaptadores_da_maquina()
        .into_iter()
        .map(|a| Gpu {
            fabricante: fabricante_da_placa(a.fabricante_id, &a.nome).to_string(),
            fabricante_id: format!("{:#06x}", a.fabricante_id),
            placa_id: format!("{:#06x}", a.placa_id),
            nome: a.nome,
            tipo: a.tipo.into(),
            backend: a.backend.into(),
            driver: if a.driver.is_empty() && a.backend == "Metal" {
                "embutido no macOS".into()
            } else {
                a.driver
            },
            driver_info: a.driver_info,
            carimbos: a.carimbos,
        })
        .collect();
    if m.gpus.is_empty() {
        m.faltas
            .push("o wgpu não enxergou nenhuma GPU (a Revelação fica sem motor)".into());
    }
    do_sistema(&mut m);
    m.coleta_ms = comeco.elapsed().as_millis() as u64;
    m
}

/// O nome do fabricante pelo id PCI.
pub fn nome_do_fabricante(id: u32) -> &'static str {
    match id {
        0x10DE => "NVIDIA",
        0x1002 | 0x1022 => "AMD",
        0x8086 => "Intel",
        0x106B => "Apple",
        0x5143 => "Qualcomm",
        0x13B5 => "ARM",
        0x1414 => "Microsoft (driver genérico)",
        0x10005 => "Mesa (software)",
        0 => "?",
        _ => "outro",
    }
}

/// O fabricante pelo id PCI e, sem ele (o Metal não diz), pelo nome.
pub fn fabricante_da_placa(id: u32, nome: &str) -> &'static str {
    let conhecido = nome_do_fabricante(id);
    if id != 0 {
        return conhecido;
    }
    let n = nome.to_lowercase();
    if n.contains("apple") {
        "Apple"
    } else if n.contains("nvidia") || n.contains("geforce") {
        "NVIDIA"
    } else if n.contains("amd") || n.contains("radeon") {
        "AMD"
    } else if n.contains("intel") {
        "Intel"
    } else {
        conhecido
    }
}

fn area_de_trabalho() -> Option<String> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let sessao = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let area = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let texto = format!("{area} ({sessao})");
    (!area.is_empty() || !sessao.is_empty()).then_some(texto)
}

/// Roda um comando com prazo, sem janela de console no Windows. `None` se não
/// existir, falhar ou passar do prazo.
fn rodar(programa: &str, argumentos: &[&str], prazo: Duration) -> Option<String> {
    let mut comando = Command::new(programa);
    comando
        .args(argumentos)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: o PowerShell não pisca uma janela preta no balcão.
        comando.creation_flags(0x0800_0000);
    }
    let mut filho = comando.spawn().ok()?;
    let fim = Instant::now() + prazo;
    loop {
        match filho.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < fim => std::thread::sleep(Duration::from_millis(25)),
            _ => {
                let _ = filho.kill();
                let _ = filho.wait();
                return None;
            }
        }
    }
    let saida = filho.wait_with_output().ok()?;
    saida
        .status
        .success()
        .then(|| String::from_utf8_lossy(&saida.stdout).into_owned())
}

/// "1352 x 878 @ 120.00Hz" → 120.
fn hz_do_texto(texto: &str) -> Option<f32> {
    let depois = texto.split('@').nth(1)?;
    let numero: String = depois
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    numero.parse().ok().filter(|hz: &f32| *hz > 1.0)
}

#[cfg(target_os = "macos")]
fn do_sistema(m: &mut Maquina) {
    match rodar(
        "system_profiler",
        &["SPDisplaysDataType", "-json"],
        Duration::from_secs(8),
    )
    .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
    {
        Some(json) => ler_system_profiler(&json, m),
        None => m
            .faltas
            .push("system_profiler não respondeu: sem monitores nem Hz do sistema".into()),
    }
    m.energia = rodar("pmset", &["-g", "ps"], Duration::from_secs(3)).map(|t| {
        if t.contains("'AC Power'") {
            "na tomada".to_string()
        } else if t.contains("'Battery Power'") {
            "na bateria".to_string()
        } else {
            t.lines().next().unwrap_or_default().trim().to_string()
        }
    });
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn ler_system_profiler(json: &serde_json::Value, m: &mut Maquina) {
    let texto = |v: &serde_json::Value, k: &str| {
        v.get(k)
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string()
    };
    for placa in json["SPDisplaysDataType"].as_array().into_iter().flatten() {
        m.placas_no_sistema.push(PlacaNoSistema {
            nome: texto(placa, "sppci_model"),
            // No macOS o driver de vídeo vem com o sistema.
            driver_versao: format!("do macOS {}", m.sistema_versao),
            driver_data: String::new(),
            memoria_mb: None,
            detalhe: format!(
                "{} núcleos de GPU · {}",
                texto(placa, "sppci_cores"),
                texto(placa, "spdisplays_mtlgpufamilysupport").replace("spdisplays_", "")
            ),
        });
        for tela in placa["spdisplays_ndrvs"].as_array().into_iter().flatten() {
            let resolucao = texto(tela, "_spdisplays_resolution");
            m.monitores.push(Monitor {
                nome: texto(tela, "_name"),
                hz: hz_do_texto(&resolucao),
                resolucao: format!("{} ({} px)", resolucao, texto(tela, "_spdisplays_pixels")),
                principal: texto(tela, "spdisplays_main") == "spdisplays_yes",
            });
        }
    }
}

#[cfg(target_os = "windows")]
fn do_sistema(m: &mut Maquina) {
    // Uma chamada só ao PowerShell: as placas (com versão e data do driver e a
    // taxa atual), a bateria e o plano de energia.
    const SCRIPT: &str = "$ErrorActionPreference='SilentlyContinue';\
        $v=Get-CimInstance Win32_VideoController | Select-Object Name,DriverVersion,DriverDate,AdapterRAM,CurrentHorizontalResolution,CurrentVerticalResolution,CurrentRefreshRate,VideoProcessor;\
        $b=Get-CimInstance Win32_Battery | Select-Object BatteryStatus,EstimatedChargeRemaining;\
        $p=(powercfg /getactivescheme) -join ' ';\
        @{video=@($v);bateria=@($b);plano=$p} | ConvertTo-Json -Depth 4 -Compress";
    let Some(json) = rodar(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", SCRIPT],
        Duration::from_secs(12),
    )
    .and_then(|t| serde_json::from_str::<serde_json::Value>(t.trim()).ok()) else {
        m.faltas
            .push("o PowerShell não respondeu: sem versão do driver nem Hz do sistema".into());
        return;
    };
    ler_cim(&json, m);
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn ler_cim(json: &serde_json::Value, m: &mut Maquina) {
    let texto = |v: &serde_json::Value, k: &str| match &v[k] {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => String::new(),
    };
    let numero = |v: &serde_json::Value, k: &str| v[k].as_f64();
    for placa in json["video"].as_array().into_iter().flatten() {
        let hz = numero(placa, "CurrentRefreshRate").map(|h| h as f32);
        let (w, h) = (
            numero(placa, "CurrentHorizontalResolution"),
            numero(placa, "CurrentVerticalResolution"),
        );
        m.placas_no_sistema.push(PlacaNoSistema {
            nome: texto(placa, "Name"),
            driver_versao: texto(placa, "DriverVersion"),
            driver_data: texto(placa, "DriverDate"),
            // `AdapterRAM` é u32 no CIM: passa de 4 GB e volta ao zero. É
            // um piso, e a tela diz "≥".
            memoria_mb: numero(placa, "AdapterRAM").map(|b| b as u64 / (1024 * 1024)),
            detalhe: texto(placa, "VideoProcessor"),
        });
        if let (Some(w), Some(h)) = (w, h) {
            m.monitores.push(Monitor {
                nome: format!("ligado a {}", texto(placa, "Name")),
                resolucao: format!("{w:.0} x {h:.0}"),
                hz: hz.filter(|h| *h > 1.0),
                principal: m.monitores.is_empty(),
            });
        }
    }
    let bateria = json["bateria"].as_array().and_then(|b| b.first().cloned());
    let plano = texto(json, "plano");
    let plano = plano
        .split_once('(')
        .map(|(_, r)| r.trim_end_matches([')', ' ']).to_string())
        .unwrap_or(plano);
    m.energia = Some(match bateria {
        // 2 = na tomada; 1 = descarregando.
        Some(b) if numero(&b, "BatteryStatus") == Some(1.0) => {
            format!("na bateria · plano {plano}")
        }
        Some(_) => format!("na tomada · plano {plano}"),
        None => format!("sem bateria · plano {plano}"),
    });
}

#[cfg(all(unix, not(target_os = "macos")))]
fn do_sistema(m: &mut Maquina) {
    // Driver: o proprietário da NVIDIA diz a versão; nos outros (amdgpu, i915,
    // nouveau) o driver do kernel é o do kernel, e o Mesa aparece no
    // `driver_info` do wgpu (Vulkan).
    for (caminho, nome) in [
        ("/proc/driver/nvidia/version", "NVIDIA (proprietário)"),
        ("/sys/module/nvidia/version", "NVIDIA (módulo)"),
    ] {
        if let Ok(t) = std::fs::read_to_string(caminho) {
            m.placas_no_sistema.push(PlacaNoSistema {
                nome: nome.into(),
                driver_versao: t.lines().next().unwrap_or_default().trim().to_string(),
                ..Default::default()
            });
            break;
        }
    }
    for modulo in ["amdgpu", "i915", "xe", "nouveau", "radeon"] {
        if std::path::Path::new(&format!("/sys/module/{modulo}")).exists() {
            m.placas_no_sistema.push(PlacaNoSistema {
                nome: format!("módulo {modulo}"),
                driver_versao: format!("do kernel {}", m.kernel),
                ..Default::default()
            });
        }
    }
    // Monitores e Hz: o `xrandr` responde no X11 (e pelo XWayland, nem sempre
    // com a taxa real). No Wayland puro não há consulta padrão: a taxa é
    // estimada pelos quadros.
    match rodar("xrandr", &["--current"], Duration::from_secs(3)) {
        Some(t) => ler_xrandr(&t, m),
        None => m.faltas.push(
            "sem xrandr (Wayland ou não instalado): a taxa do monitor é estimada pelos quadros"
                .into(),
        ),
    }
    let mut fontes = Vec::new();
    if let Ok(lista) = std::fs::read_dir("/sys/class/power_supply") {
        for e in lista.flatten() {
            let p = e.path();
            let tipo = std::fs::read_to_string(p.join("type")).unwrap_or_default();
            if tipo.trim() == "Mains" {
                let online = std::fs::read_to_string(p.join("online")).unwrap_or_default();
                fontes.push(online.trim() == "1");
            }
        }
    }
    m.energia = match fontes.as_slice() {
        [] => None,
        f if f.iter().any(|x| *x) => Some("na tomada".into()),
        _ => Some("na bateria".into()),
    };
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
fn ler_xrandr(texto: &str, m: &mut Maquina) {
    let mut atual: Option<(String, bool)> = None;
    for linha in texto.lines() {
        if !linha.starts_with(' ') && linha.contains(" connected") {
            let nome = linha
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string();
            atual = Some((nome, linha.contains(" primary ")));
        } else if linha.starts_with("   ") && linha.contains('*') {
            if let Some((nome, principal)) = atual.take() {
                let mut partes = linha.split_whitespace();
                let resolucao = partes.next().unwrap_or_default().to_string();
                let hz = partes
                    .find(|p| p.contains('*'))
                    .and_then(|p| p.trim_end_matches(['*', '+']).parse::<f32>().ok());
                m.monitores.push(Monitor {
                    nome,
                    resolucao,
                    hz,
                    principal,
                });
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_taxa_sai_do_texto_do_system_profiler() {
        assert_eq!(hz_do_texto("1352 x 878 @ 120.00Hz"), Some(120.0));
        assert_eq!(hz_do_texto("1920 x 1080 @ 75.00Hz"), Some(75.0));
        assert_eq!(hz_do_texto("1920 x 1080"), None);
    }

    #[test]
    fn o_system_profiler_da_placa_monitores_e_o_principal() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"SPDisplaysDataType":[{"sppci_model":"Apple M2 Pro","sppci_cores":"16",
            "spdisplays_mtlgpufamilysupport":"spdisplays_metal4","spdisplays_ndrvs":[
            {"_name":"Color LCD","_spdisplays_resolution":"1352 x 878 @ 120.00Hz",
             "_spdisplays_pixels":"2704 x 1756","_spdisplays_display-serial-number":"segredo",
             "spdisplays_main":"spdisplays_yes"},
            {"_name":"LG FULL HD","_spdisplays_resolution":"1920 x 1080 @ 75.00Hz",
             "_spdisplays_pixels":"1920 x 1080"}]}]}"#,
        )
        .unwrap();
        let mut m = Maquina::default();
        ler_system_profiler(&json, &mut m);
        assert_eq!(m.placas_no_sistema[0].nome, "Apple M2 Pro");
        assert_eq!(m.monitores.len(), 2);
        assert_eq!(m.hz_do_principal(), Some(120.0));
        let tudo = serde_json::to_string(&m).unwrap();
        assert!(!tudo.contains("segredo"), "número de série não vai");
    }

    #[test]
    fn o_cim_do_windows_da_driver_hz_e_energia() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"video":[{"Name":"NVIDIA GeForce RTX 3060","DriverVersion":"32.0.15.6094",
            "DriverDate":"20240812","AdapterRAM":4293918720,"CurrentHorizontalResolution":2560,
            "CurrentVerticalResolution":1440,"CurrentRefreshRate":144,"VideoProcessor":"GA106"}],
            "bateria":[],"plano":"GUID do Esquema de Energia: 381b...  (Equilibrado)"}"#,
        )
        .unwrap();
        let mut m = Maquina::default();
        ler_cim(&json, &mut m);
        assert_eq!(m.placas_no_sistema[0].driver_versao, "32.0.15.6094");
        assert_eq!(m.hz_do_principal(), Some(144.0));
        assert_eq!(
            m.energia.as_deref(),
            Some("sem bateria · plano Equilibrado")
        );
    }

    #[test]
    fn o_xrandr_da_a_taxa_do_modo_atual() {
        let mut m = Maquina::default();
        ler_xrandr(
            "Screen 0: minimum 8 x 8\n\
             eDP-1 connected primary 1920x1080+0+0 (normal) 344mm x 194mm\n\
             \x20  1920x1080     60.01*+  59.93    48.00\n\
             \x20  1680x1050     59.95\n\
             HDMI-1 connected 2560x1440+1920+0\n\
             \x20  2560x1440    143.97*   59.95\n\
             DP-1 disconnected\n",
            &mut m,
        );
        assert_eq!(m.monitores.len(), 2);
        assert_eq!(m.hz_do_principal(), Some(60.01));
        assert_eq!(m.monitores[1].hz, Some(143.97));
    }
}

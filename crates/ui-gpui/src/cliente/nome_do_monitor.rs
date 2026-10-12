//! 🏷️ **O nome do monitor**, como o sistema o chama: `DELL U2720Q`, `Tela
//! Retina Integrada`.
//!
//! # Por que existe
//!
//! A escolha da tela do cliente dizia só "Monitor 2 · 1920 × 1080", e com dois
//! monitores do mesmo tamanho o número não diz qual é qual (dono,
//! 11/out/2026). O nome é o que está escrito na moldura e nas configurações de
//! tela do sistema.
//!
//! # De onde vem
//!
//! O GPUI não o expõe (`PlatformDisplay` só tem `id`, `uuid` e os limites),
//! então cada sistema é perguntado direto, a partir do que o GPUI dá:
//!
//! | sistema | chave | fonte |
//! |---|---|---|
//! | macOS | o `id` é o `CGDirectDisplayID` | `NSScreen.localizedName` |
//! | Windows | o `id` é o `HMONITOR` | `QueryDisplayConfig` (nome amigável) |
//! | Linux (Wayland) | o `uuid` sai do nome do conector | EDID em `/sys/class/drm` |
//!
//! Sem nome (X11, painel sem EDID, sistema que não respondeu) devolve `None`,
//! e o botão fica como era.

/// O nome que o sistema dá ao monitor, ou `None` se ele não disser.
///
/// `id` é o `DisplayId` do GPUI em número, e `uuid` o `PlatformDisplay::uuid`.
pub fn do_sistema(id: u64, uuid: Option<&str>) -> Option<String> {
    plataforma::nome(id, uuid).and_then(limpo)
}

/// Sem espaço nas pontas, e vazio vira `None`: o painel do notebook, no
/// Windows, responde com um nome em branco.
fn limpo(nome: String) -> Option<String> {
    let nome = nome.trim();
    (!nome.is_empty()).then(|| nome.to_string())
}

/// O nome do monitor dentro do EDID: o descritor `0xFC` ("display product
/// name"), um dos quatro blocos de 18 bytes a partir do byte 54.
///
/// O texto tem até 13 bytes e acaba em `\n`, completado com espaços.
#[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
fn nome_do_edid(edid: &[u8]) -> Option<String> {
    const NOME_DO_PRODUTO: u8 = 0xFC;
    (0..4)
        .filter_map(|i| edid.get(54 + i * 18..72 + i * 18))
        .find(|bloco| bloco[..3] == [0, 0, 0] && bloco[3] == NOME_DO_PRODUTO)
        .and_then(|bloco| {
            let texto: String = bloco[5..]
                .iter()
                .take_while(|b| **b != b'\n' && **b != 0)
                .map(|b| *b as char)
                .collect();
            limpo(texto)
        })
}

/// O conector de uma pasta de `/sys/class/drm`: `card1-HDMI-A-1` → `HDMI-A-1`.
/// As pastas que não são conector (`card1`, `renderD128`, `version`) dão `None`.
#[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
fn conector_da_pasta(pasta: &str) -> Option<&str> {
    let (placa, conector) = pasta.split_once('-')?;
    let numero = placa.strip_prefix("card")?;
    (!numero.is_empty() && numero.bytes().all(|b| b.is_ascii_digit())).then_some(conector)
}

/// Os nomes pelos quais o compositor pode chamar o conector.
///
/// 🚨 **O GNOME não usa o nome do kernel no HDMI**: o kernel diz `HDMI-A-1`, e
/// o Mutter anuncia `HDMI-1` no `wl_output`. O KDE usa o do kernel. E o HDMI é
/// justamente a porta do monitor do cliente.
#[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
fn apelidos_do_conector(conector: &str) -> Vec<String> {
    let mut nomes = vec![conector.to_string()];
    if let Some(resto) = conector.strip_prefix("HDMI-A-") {
        nomes.push(format!("HDMI-{resto}"));
    }
    nomes
}

#[cfg(target_os = "macos")]
mod plataforma {
    use std::ffi::{c_char, CStr};

    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};

    /// O `NSScreen` cujo `NSScreenNumber` é o `id`, pelo nome localizado — o
    /// mesmo texto de Ajustes › Telas.
    ///
    /// Só na thread principal, que é de onde o desenho da pergunta chama.
    pub(super) fn nome(id: u64, _uuid: Option<&str>) -> Option<String> {
        // SAFETY: mensagens a classes do AppKit com as assinaturas da
        // documentação; todo ponteiro é conferido antes do uso, e os objetos
        // são do pool de autorelease do laço de eventos.
        unsafe {
            let telas: *mut AnyObject = msg_send![AnyClass::get(c"NSScreen")?, screens];
            let chave: *mut AnyObject = msg_send![
                AnyClass::get(c"NSString")?,
                stringWithUTF8String: c"NSScreenNumber".as_ptr()
            ];
            if telas.is_null() || chave.is_null() {
                return None;
            }
            let quantas: usize = msg_send![telas, count];
            for i in 0..quantas {
                let tela: *mut AnyObject = msg_send![telas, objectAtIndex: i];
                let descricao: *mut AnyObject = msg_send![tela, deviceDescription];
                if descricao.is_null() {
                    continue;
                }
                let numero: *mut AnyObject = msg_send![descricao, objectForKey: chave];
                if numero.is_null() {
                    continue;
                }
                let numero: u32 = msg_send![numero, unsignedIntValue];
                if u64::from(numero) != id {
                    continue;
                }
                let nome: *mut AnyObject = msg_send![tela, localizedName];
                if nome.is_null() {
                    return None;
                }
                let texto: *const c_char = msg_send![nome, UTF8String];
                if texto.is_null() {
                    return None;
                }
                return Some(CStr::from_ptr(texto).to_string_lossy().into_owned());
            }
            None
        }
    }
}

#[cfg(target_os = "windows")]
mod plataforma {
    use std::mem::{size_of, zeroed};

    use windows_sys::Win32::Devices::Display::{
        DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig,
        DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
        DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SOURCE_DEVICE_NAME,
        DISPLAYCONFIG_TARGET_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
    };
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITORINFO, MONITORINFOEXW};

    /// O "nome amigável" do monitor — o de Configurações › Tela.
    ///
    /// O `HMONITOR` dá o dispositivo do GDI (`\\.\DISPLAY2`); a configuração de
    /// telas liga esse dispositivo ao monitor plugado nele, e é o monitor que
    /// tem nome.
    pub(super) fn nome(id: u64, _uuid: Option<&str>) -> Option<String> {
        // SAFETY: estruturas zeradas com o tamanho declarado, como a API pede,
        // e vetores do tamanho que o próprio sistema informou.
        unsafe {
            let mut monitor: MONITORINFOEXW = zeroed();
            monitor.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
            if GetMonitorInfoW(
                id as usize as _,
                &mut monitor as *mut MONITORINFOEXW as *mut MONITORINFO,
            ) == 0
            {
                return None;
            }

            let (mut caminhos, mut modos) = (0u32, 0u32);
            if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut caminhos, &mut modos) != 0 {
                return None;
            }
            let mut lista: Vec<DISPLAYCONFIG_PATH_INFO> = vec![zeroed(); caminhos as usize];
            let mut lista_de_modos: Vec<DISPLAYCONFIG_MODE_INFO> = vec![zeroed(); modos as usize];
            if QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut caminhos,
                lista.as_mut_ptr(),
                &mut modos,
                lista_de_modos.as_mut_ptr(),
                std::ptr::null_mut(),
            ) != 0
            {
                return None;
            }

            for caminho in lista.iter().take(caminhos as usize) {
                let mut origem: DISPLAYCONFIG_SOURCE_DEVICE_NAME = zeroed();
                origem.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
                origem.header.size = size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
                origem.header.adapterId = caminho.sourceInfo.adapterId;
                origem.header.id = caminho.sourceInfo.id;
                if DisplayConfigGetDeviceInfo(&mut origem.header) != 0
                    || origem.viewGdiDeviceName != monitor.szDevice
                {
                    continue;
                }

                let mut alvo: DISPLAYCONFIG_TARGET_DEVICE_NAME = zeroed();
                alvo.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
                alvo.header.size = size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
                alvo.header.adapterId = caminho.targetInfo.adapterId;
                alvo.header.id = caminho.targetInfo.id;
                if DisplayConfigGetDeviceInfo(&mut alvo.header) != 0 {
                    continue;
                }
                let nome = &alvo.monitorFriendlyDeviceName;
                let fim = nome.iter().position(|c| *c == 0).unwrap_or(nome.len());
                return Some(String::from_utf16_lossy(&nome[..fim]));
            }
            None
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod plataforma {
    use uuid::Uuid;

    /// O conector plugado cujo nome dá o `uuid`, pelo EDID dele.
    ///
    /// No Wayland o GPUI faz o `uuid` do nome do `wl_output` (`DP-1`), que é o
    /// conector; o kernel guarda o EDID de cada conector em `/sys/class/drm`.
    /// Monitor sem nome no EDID (painel de notebook) fica com o do conector.
    /// No X11 o GPUI dá um "monitor" por tela do X e um `uuid` zerado: nada
    /// casa, e a pergunta nem chega a abrir.
    pub(super) fn nome(_id: u64, uuid: Option<&str>) -> Option<String> {
        let uuid = uuid?;
        for entrada in std::fs::read_dir("/sys/class/drm").ok()?.flatten() {
            let pasta = entrada.file_name();
            let pasta = pasta.to_string_lossy();
            let Some(conector) = super::conector_da_pasta(&pasta) else {
                continue;
            };
            let Some(apelido) = super::apelidos_do_conector(conector)
                .into_iter()
                .find(|nome| {
                    Uuid::new_v5(&Uuid::NAMESPACE_DNS, nome.as_bytes()).to_string() == uuid
                })
            else {
                continue;
            };
            // Com duas placas pode haver dois `HDMI-A-1`: vale o plugado.
            let plugado = std::fs::read_to_string(entrada.path().join("status"))
                .is_ok_and(|estado| estado.trim() == "connected");
            if !plugado {
                continue;
            }
            let edid = std::fs::read(entrada.path().join("edid")).unwrap_or_default();
            return super::nome_do_edid(&edid).or(Some(apelido));
        }
        None
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Um EDID de 128 bytes com `nome` no segundo descritor.
    fn edid_com(nome: &[u8]) -> Vec<u8> {
        let mut edid = vec![0u8; 128];
        // O primeiro descritor é um modo de vídeo: começa com o relógio, não zero.
        edid[54] = 0x02;
        edid[55] = 0x3A;
        let bloco = &mut edid[72..90];
        bloco[3] = 0xFC;
        bloco[5..].fill(b' ');
        bloco[5..5 + nome.len()].copy_from_slice(nome);
        if nome.len() < 13 {
            bloco[5 + nome.len()] = b'\n';
        }
        edid
    }

    #[test]
    fn o_nome_sai_do_descritor_de_produto_do_edid() {
        assert_eq!(
            nome_do_edid(&edid_com(b"DELL U2720Q")).as_deref(),
            Some("DELL U2720Q")
        );
        // 13 letras ocupam o bloco inteiro, sem o `\n` do fim.
        assert_eq!(
            nome_do_edid(&edid_com(b"LG ULTRAWIDE1")).as_deref(),
            Some("LG ULTRAWIDE1")
        );
    }

    /// Painel de notebook costuma não ter o descritor; e o arquivo do `/sys`
    /// vem vazio com o conector desplugado.
    #[test]
    fn edid_sem_nome_ou_curto_nao_da_nome() {
        assert_eq!(nome_do_edid(&[0u8; 128]), None);
        assert_eq!(nome_do_edid(&[]), None);
        assert_eq!(nome_do_edid(&edid_com(b"")), None);
        assert_eq!(nome_do_edid(&edid_com(b"DELL")[..80]), None);
    }

    #[test]
    fn so_pasta_de_conector_vira_conector() {
        assert_eq!(conector_da_pasta("card1-HDMI-A-1"), Some("HDMI-A-1"));
        assert_eq!(conector_da_pasta("card0-eDP-1"), Some("eDP-1"));
        assert_eq!(conector_da_pasta("card1"), None);
        assert_eq!(conector_da_pasta("renderD128"), None);
        assert_eq!(conector_da_pasta("version"), None);
    }

    /// 🚨 O GNOME chama de `HDMI-1` o que o kernel chama de `HDMI-A-1`.
    #[test]
    fn o_hdmi_tambem_atende_pelo_nome_do_gnome() {
        assert_eq!(apelidos_do_conector("HDMI-A-1"), ["HDMI-A-1", "HDMI-1"]);
        assert_eq!(apelidos_do_conector("DP-2"), ["DP-2"]);
    }

    #[test]
    fn nome_em_branco_nao_e_nome() {
        assert_eq!(limpo("  ".into()), None);
        assert_eq!(
            limpo(" DELL U2720Q ".into()).as_deref(),
            Some("DELL U2720Q")
        );
    }
}

//! Quem pode excluir uma sessão, e quando — as regras puras, do lado da tela.
//!
//! É o `exclusao.ts` e o `sobras-da-lista.ts` do site
//! (`frontend/src/app/(dashboard)/dashboard/sessoes-fotograficas/`), com as
//! mesmas frases.
//!
//! ⚠️ **Isto não é a autorização.** Quem decide é o backend, que confere a flag
//! de SuperAdmin contra o cadastro, exige a frase e recusa a sessão com venda.
//! Aqui é só o que a tela precisa para não oferecer um botão que sempre falha —
//! e um botão que sempre falha faz quem clica concluir que o sistema quebrou.
//!
//! 🗑️ **Excluir não apaga** (exclusão lógica, 2026-09-13): a sessão sai da
//! lista e do link do cliente, e volta pela aba "Excluídas".

/// A frase que o operador digita. Igual à do backend, caractere a caractere.
pub const FRASE_DE_CONFIRMACAO: &str = "CONFIRMAR EXCLUSÃO!";

/// O único e-mail que pode ser SuperAdmin — o mesmo do domínio do backend.
///
/// Uma cópia de propósito, como a do site: a que **vale** é a do Rust do
/// backend; esta só esconde o botão. Se divergirem, o pior que acontece é o
/// botão aparecer e o backend recusar com 403 — nunca o contrário.
const EMAIL_DO_SUPER_ADMIN: &str = "alexkads@gmail.com";

/// Depois disto, o relato de uma máquina é **possivelmente desatualizado** —
/// o `RELATO_VELHO_EM_MS` do site: uma semana sem ouvir dela.
pub const RELATO_VELHO_EM_S: i64 = 7 * 24 * 60 * 60;

/// É o SuperAdmin? `papel` é o `role` do `/auth/me`.
pub fn e_super_admin(email: &str, papel: Option<&str>) -> bool {
    papel == Some("ADMIN") && email.trim().eq_ignore_ascii_case(EMAIL_DO_SUPER_ADMIN)
}

/// O digitado confere com a frase? Espaço nas pontas não conta, como no site.
pub fn confere(digitado: &str) -> bool {
    digitado.trim() == FRASE_DE_CONFIRMACAO
}

/// O que impede a sessão de ser excluída.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bloqueio {
    pub levadas: u32,
    pub compradas: u32,
}

/// O que impede esta sessão de ser excluída — ou `None` quando nada impede.
///
/// Foto paga, no balcão ou no pós-venda, tranca a sessão. Foto à venda não
/// tranca — ninguém pagou por ela.
///
/// ⚠️ **A contagem da lista não traz as apagadas pela retenção**, e o backend
/// conta elas também. A tela pode liberar o botão para uma sessão que o backend
/// recusa — e é o lado certo para errar: o `409` volta com a frase dizendo
/// quantas são.
pub fn bloqueio(levadas_no_balcao: u32, compradas: u32) -> Option<Bloqueio> {
    (levadas_no_balcao + compradas > 0).then_some(Bloqueio {
        levadas: levadas_no_balcao,
        compradas,
    })
}

/// "3 levada(s) no balcão e 2 comprada(s) no pós-venda" — o texto do aviso.
pub fn descrever(b: Bloqueio) -> String {
    let mut partes = Vec::new();
    if b.levadas > 0 {
        partes.push(format!("{} levada(s) no balcão", b.levadas));
    }
    if b.compradas > 0 {
        partes.push(format!("{} comprada(s) no pós-venda", b.compradas));
    }
    partes.join(" e ")
}

/// O relato de uma máquina sobre uma sessão: quantas fotos ela guarda fora do
/// servidor, e quando disse isso (segundos desde a época).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sobra {
    pub origem: String,
    pub quantas: u32,
    pub visto_em: i64,
}

/// Uma máquina com fotos da sessão, pronta para a tela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Maquina {
    pub origem: String,
    pub quantas: u32,
    /// "há 3 dias".
    pub ha_quanto: String,
    /// O recado passou de [`RELATO_VELHO_EM_S`]: pode não valer mais.
    pub velho: bool,
}

/// As máquinas que relatam fotos da sessão, a maior primeiro. As de zero saem.
///
/// 🔑 **O app não relata as próprias sobras** — quem relata são os navegadores
/// do balcão. Então aqui não há "esta máquina" a separar, como no site: todas
/// são outras.
pub fn maquinas(sobras: &[Sobra], agora: i64) -> Vec<Maquina> {
    let mut lista: Vec<Maquina> = sobras
        .iter()
        .filter(|s| s.quantas > 0)
        .map(|s| Maquina {
            origem: s.origem.clone(),
            quantas: s.quantas,
            ha_quanto: ha_quanto(s.visto_em, agora),
            velho: agora - s.visto_em > RELATO_VELHO_EM_S,
        })
        .collect();
    lista.sort_by_key(|m| std::cmp::Reverse(m.quantas));
    lista
}

/// "agora", "há 5 min", "há 3 h", "há 2 dias", "há 3 semanas", "há 2 meses".
pub fn ha_quanto(quando: i64, agora: i64) -> String {
    let s = (agora - quando).max(0);
    if s < 60 {
        return "agora".into();
    }
    let min = s / 60;
    if min < 60 {
        return format!("há {min} min");
    }
    let h = min / 60;
    if h < 24 {
        return format!("há {h} h");
    }
    let dias = h / 24;
    if dias < 14 {
        return if dias == 1 {
            "há 1 dia".into()
        } else {
            format!("há {dias} dias")
        };
    }
    if dias < 60 {
        return format!("há {} semanas", dias / 7);
    }
    let meses = dias / 30;
    if meses < 24 {
        format!("há {meses} meses")
    } else {
        format!("há {} anos", dias / 365)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn so_o_admin_com_o_email_do_dono_e_super_admin() {
        assert!(e_super_admin("alexkads@gmail.com", Some("ADMIN")));
        assert!(e_super_admin("  AlexKads@Gmail.com ", Some("ADMIN")));
        assert!(!e_super_admin("alexkads@gmail.com", Some("USER")));
        assert!(!e_super_admin("alexkads@gmail.com", None));
        assert!(!e_super_admin("outro@gmail.com", Some("ADMIN")));
    }

    #[test]
    fn a_frase_confere_so_inteira() {
        assert!(confere("CONFIRMAR EXCLUSÃO!"));
        assert!(confere("  CONFIRMAR EXCLUSÃO! "));
        assert!(!confere("confirmar exclusão!"));
        assert!(!confere("CONFIRMAR EXCLUSAO!"));
        assert!(!confere("CONFIRMAR EXCLUSÃO"));
    }

    #[test]
    fn foto_paga_tranca_e_foto_a_venda_nao() {
        assert_eq!(bloqueio(0, 0), None);
        assert_eq!(
            bloqueio(3, 0),
            Some(Bloqueio {
                levadas: 3,
                compradas: 0
            })
        );
        assert_eq!(
            descrever(bloqueio(3, 2).unwrap()),
            "3 levada(s) no balcão e 2 comprada(s) no pós-venda"
        );
        assert_eq!(
            descrever(bloqueio(0, 1).unwrap()),
            "1 comprada(s) no pós-venda"
        );
    }

    #[test]
    fn as_maquinas_vem_da_maior_e_a_de_uma_semana_e_velha() {
        let agora = 1_000_000_000;
        let lista = maquinas(
            &[
                Sobra {
                    origem: "Balcão 1".into(),
                    quantas: 4,
                    visto_em: agora - 120,
                },
                Sobra {
                    origem: "Vazia".into(),
                    quantas: 0,
                    visto_em: agora,
                },
                Sobra {
                    origem: "Notebook".into(),
                    quantas: 30,
                    visto_em: agora - 8 * 24 * 3600,
                },
            ],
            agora,
        );
        assert_eq!(lista.len(), 2);
        assert_eq!(lista[0].origem, "Notebook");
        assert!(lista[0].velho);
        assert_eq!(lista[0].ha_quanto, "há 8 dias");
        assert_eq!(lista[1].ha_quanto, "há 2 min");
        assert!(!lista[1].velho);
    }

    #[test]
    fn ha_quanto_como_no_site() {
        let h = |s: i64| ha_quanto(0, s);
        assert_eq!(h(30), "agora");
        assert_eq!(h(3 * 3600), "há 3 h");
        assert_eq!(h(24 * 3600), "há 1 dia");
        assert_eq!(h(21 * 24 * 3600), "há 3 semanas");
        assert_eq!(h(90 * 24 * 3600), "há 3 meses");
        assert_eq!(h(800 * 24 * 3600), "há 2 anos");
    }
}

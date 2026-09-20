//! Os dados do cliente de uma sessão — título, e-mail e WhatsApp — em regras
//! puras. É o `contato-da-sessao.ts` do site, portado.
//!
//! # O contato é exigido no fim da sessão, e não no começo
//!
//! *"Não tem que obrigar o email e whatsapp para criar a sessão. Essas
//! informações são obrigatórias no final da sessão."* — dono, 2026-09-13.
//!
//! A sessão nasce só com o título. Quem pede é o fim: "Copiar link" e "Avisar"
//! numa sessão **sem e-mail** abrem o formulário em [`Modo::PedirEmail`], e o
//! site recusa com `422` se chegar lá sem.
//!
//! 🚨 **No fim é o e-mail, e não "qualquer contato"** (usuários em produção,
//! 2026-09-13: *"não consigo gerar link"*). O link entra na conta daquele
//! e-mail, e o aviso vai por e-mail. Só com WhatsApp nenhum dos dois sai.
//!
//! *"Dentro da sessão precisa ser possível mudar o Título, email e o
//! whatsapp."* — dono, 2026-09-13. O mesmo formulário, em [`Modo::Editar`],
//! deixa apagar e-mail e WhatsApp.
//!
//! 🔑 **Uma conferência por plataforma, e as duas iguais**: a mesma frase, o
//! mesmo "e-mail plausível" (`^[^\s@]+@[^\s@]+\.[^\s@]+$` no site). E-mail
//! preenchido pela metade é recusado nos dois modos — ele não melhora sozinho,
//! e gravado seria a galeria que ninguém abre.

/// O limite do site (`titulo_valido` no backend).
pub const TAMANHO_MAXIMO_DO_TITULO: usize = 120;

pub const FALTA_TITULO: &str = "Informe o título.";
pub const TITULO_LONGO: &str = "O título tem no máximo 120 caracteres.";
pub const EMAIL_INCOMPLETO: &str = "O e-mail não parece completo.";
pub const FALTA_EMAIL: &str = "Informe o e-mail do cliente.";

/// Para que o formulário foi aberto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    /// Mudar título, e-mail e WhatsApp. O contato pode ficar vazio.
    Editar,
    /// O fim da sessão (link, aviso) pediu o e-mail: sem ele, não grava. O
    /// WhatsApp continua opcional.
    PedirEmail,
}

/// Qual campo a recusa aponta — para a tela pôr a frase ao lado dele.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Campo {
    Titulo,
    Email,
    Whatsapp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recusa {
    pub campo: Campo,
    pub frase: &'static str,
}

/// Os dados como ficam — vazio já é `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DadosDoCliente {
    pub titulo: String,
    pub email: Option<String>,
    pub whatsapp: Option<String>,
}

/// O que o `PATCH /galerias/{id}` leva: só o que mudou.
///
/// `None` = não mexer; `Some(None)` = apagar; `Some(Some(v))` = gravar `v`. O
/// título é `Option` simples porque não se apaga.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mudancas {
    pub titulo: Option<String>,
    pub email: Option<Option<String>>,
    pub whatsapp: Option<Option<String>>,
}

impl Mudancas {
    /// Nada mudou — não se gasta uma ida à rede.
    pub fn vazia(&self) -> bool {
        self.titulo.is_none() && self.email.is_none() && self.whatsapp.is_none()
    }
}

/// Campo em branco é ausente.
pub fn preenchido(valor: &str) -> Option<String> {
    let limpo = valor.trim();
    (!limpo.is_empty()).then(|| limpo.to_string())
}

/// O mesmo teste do site: algo, `@`, algo, `.`, algo — sem espaço e sem um
/// segundo `@`.
pub fn email_plausivel(email: &str) -> bool {
    let e = email.trim();
    if e.is_empty() || e.chars().any(char::is_whitespace) {
        return false;
    }
    let mut partes = e.split('@');
    let (Some(local), Some(dominio), None) = (partes.next(), partes.next(), partes.next()) else {
        return false;
    };
    // `[^\s@]+\.[^\s@]+`: um ponto com ao menos um caractere antes e depois.
    !local.is_empty()
        && dominio
            .char_indices()
            .any(|(i, c)| c == '.' && i > 0 && i + 1 < dominio.len())
}

/// A sessão tem e-mail — o que o link e o aviso exigem?
pub fn tem_email(email: Option<&str>) -> bool {
    email.is_some_and(|v| !v.trim().is_empty())
}

/// Confere o que foi digitado, na ordem em que a tela mostra os campos.
pub fn conferir(
    modo: Modo,
    titulo: &str,
    email: &str,
    whatsapp: &str,
) -> Result<DadosDoCliente, Recusa> {
    let titulo = titulo.trim();
    if titulo.is_empty() {
        return Err(Recusa {
            campo: Campo::Titulo,
            frase: FALTA_TITULO,
        });
    }
    if titulo.chars().count() > TAMANHO_MAXIMO_DO_TITULO {
        return Err(Recusa {
            campo: Campo::Titulo,
            frase: TITULO_LONGO,
        });
    }
    let email = preenchido(email);
    let whatsapp = preenchido(whatsapp);
    if modo == Modo::PedirEmail && email.is_none() {
        return Err(Recusa {
            campo: Campo::Email,
            frase: FALTA_EMAIL,
        });
    }
    if email.as_deref().is_some_and(|e| !email_plausivel(e)) {
        return Err(Recusa {
            campo: Campo::Email,
            frase: EMAIL_INCOMPLETO,
        });
    }
    Ok(DadosDoCliente {
        titulo: titulo.to_string(),
        email,
        whatsapp,
    })
}

/// Só o que mudou entre o que a sessão tem e o que foi conferido.
pub fn mudancas(atual: &DadosDoCliente, novo: &DadosDoCliente) -> Mudancas {
    Mudancas {
        titulo: (novo.titulo != atual.titulo.trim()).then(|| novo.titulo.clone()),
        email: (novo.email.as_deref() != atual.email.as_deref().map(str::trim))
            .then(|| novo.email.clone()),
        whatsapp: (novo.whatsapp.as_deref() != atual.whatsapp.as_deref().map(str::trim))
            .then(|| novo.whatsapp.clone()),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn atual() -> DadosDoCliente {
        DadosDoCliente {
            titulo: "Ensaio da Ana".into(),
            email: Some("ana@x.com".into()),
            whatsapp: None,
        }
    }

    #[test]
    fn o_email_plausivel_e_o_mesmo_do_site() {
        assert!(email_plausivel("ana@x.com"));
        assert!(email_plausivel("  ana.souza@estudio.com.br "));
        assert!(!email_plausivel("ana@"));
        assert!(!email_plausivel("ana@x"));
        assert!(!email_plausivel("ana@x."));
        assert!(!email_plausivel("@x.com"));
        assert!(!email_plausivel("ana@@x.com"));
        assert!(!email_plausivel("ana souza@x.com"));
    }

    /// 🔚 Editar aceita sem contato; o pedido do fim exige o e-mail — **mesmo
    /// com WhatsApp**, porque o link entra na conta do e-mail.
    #[test]
    fn editar_aceita_sem_contato_e_pedir_email_exige_o_email() {
        let editado = conferir(Modo::Editar, "Ensaio", "", " ").unwrap();
        assert_eq!((editado.email, editado.whatsapp), (None, None));

        let recusa = Recusa {
            campo: Campo::Email,
            frase: FALTA_EMAIL,
        };
        assert_eq!(conferir(Modo::PedirEmail, "Ensaio", "", ""), Err(recusa));
        assert_eq!(
            conferir(Modo::PedirEmail, "Ensaio", "", "(47) 99999-8888"),
            Err(recusa),
            "só WhatsApp não basta"
        );
        let com_email =
            conferir(Modo::PedirEmail, "Ensaio", "ana@x.com", "(47) 99999-8888").unwrap();
        assert_eq!(com_email.whatsapp.as_deref(), Some("(47) 99999-8888"));
    }

    #[test]
    fn titulo_vazio_ou_longo_e_email_pela_metade_sao_recusados_nos_dois_modos() {
        for modo in [Modo::Editar, Modo::PedirEmail] {
            assert_eq!(
                conferir(modo, "  ", "ana@x.com", "").unwrap_err().campo,
                Campo::Titulo
            );
            let longo = "a".repeat(TAMANHO_MAXIMO_DO_TITULO + 1);
            assert_eq!(
                conferir(modo, &longo, "ana@x.com", "").unwrap_err().frase,
                TITULO_LONGO
            );
            assert_eq!(
                conferir(modo, "Ensaio", "ana@", "(47) 99999-8888"),
                Err(Recusa {
                    campo: Campo::Email,
                    frase: EMAIL_INCOMPLETO
                })
            );
        }
    }

    #[test]
    fn so_o_que_mudou_vai_no_patch() {
        let igual = conferir(Modo::Editar, " Ensaio da Ana ", "ana@x.com", "").unwrap();
        assert!(mudancas(&atual(), &igual).vazia());

        let titulo = conferir(Modo::Editar, "Ensaio da Ana e do João", "ana@x.com", "").unwrap();
        assert_eq!(
            mudancas(&atual(), &titulo),
            Mudancas {
                titulo: Some("Ensaio da Ana e do João".into()),
                ..Default::default()
            }
        );

        // Apagar o último contato é `Some(None)` — e não "não mexer".
        let sem_contato = conferir(Modo::Editar, "Ensaio da Ana", "", "").unwrap();
        assert_eq!(
            mudancas(&atual(), &sem_contato),
            Mudancas {
                email: Some(None),
                ..Default::default()
            }
        );

        let whatsapp =
            conferir(Modo::Editar, "Ensaio da Ana", "ana@x.com", "5547999998888").unwrap();
        assert_eq!(
            mudancas(&atual(), &whatsapp).whatsapp,
            Some(Some("5547999998888".into()))
        );
    }

    #[test]
    fn tem_email_trata_branco_como_ausente() {
        assert!(!tem_email(None));
        assert!(!tem_email(Some("  ")));
        assert!(tem_email(Some("ana@x.com")));
    }
}

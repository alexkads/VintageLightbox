//! O que a conta logada pode — a regra pura, do lado da tela.
//!
//! É o `acesso` que `GET /auth/me` devolve desde os **perfis de acesso** do
//! site (dono, 11/out/2026: *"definir qual estúdio o usuário pode ter acesso ao
//! Vintagelightbox"*, com o perfil cadastrado em `/dashboard/perfis-de-acesso`).
//! O contrato está em `docs/PERFIS_DE_ACESSO.md` do `recordarfotos-e-commerce`.
//!
//! ⚠️ **Isto não é a autorização.** Quem decide é o backend, a cada requisição:
//! o que o perfil não tem responde `403`, e sessão de outro estúdio também.
//! Aqui é só o que a tela precisa para não oferecer o que sempre falharia —
//! como em [`crate::exclusao`].
//!
//! 🔑 **Sem `acesso` na resposta, nada se esconde.** É a API de antes dos
//! perfis (ou uma resposta que não deu para ler): esconder por dúvida tiraria
//! do balcão uma tela que o backend entregaria.

/// Operar o balcão: sessões, fotos e revelação. É a que abre o app.
pub const OPERAR_O_BALCAO: &str = "OperateCounter";
/// Mexer no caixa: abrir, vender, movimentar, conferir e fechar.
pub const CAIXA: &str = "OperateCashier";
/// Estornar uma venda.
pub const ESTORNAR: &str = "RefundSales";
/// Cadastrar e alterar funcionários.
pub const FUNCIONARIOS: &str = "ManageStaff";
/// A agenda dos clientes.
pub const AGENDA: &str = "ManageBookings";
/// As conversas (WhatsApp, Instagram, chat do site).
pub const CONVERSAS: &str = "ManageWhatsApp";
/// O acervo inteiro de arquivos — o "Backup de arquivos".
pub const ACERVO: &str = "ManageMedia";

/// O que a conta pode, como o `/auth/me` contou.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Acesso {
    /// O papel `ADMIN`: tudo, em todos os estúdios.
    pub admin: bool,
    /// O nome do perfil de acesso da conta, quando ela tem um.
    pub perfil: Option<String>,
    /// `false` = o perfil existe e está desativado: não concede nada.
    pub perfil_ativo: bool,
    /// Os nomes das capacidades, como o backend os chama.
    pub permissoes: Vec<String>,
    /// Os estúdios que a conta alcança. **`None` = todos.**
    pub estudios: Option<Vec<String>>,
    /// 📍 Em qual local a sessão nasce (18/out/2026).
    pub local_padrao: crate::local::LocalDaSessao,
    /// 📍 Pode escolher outro local e corrigir o local depois.
    pub pode_mudar_local: bool,
    /// 📍 A lista de sessões fica no local padrão, sem o seletor de local — o
    /// perfil manda, a conta não muda o local, ou esconde um local (o backend
    /// junta as três razões).
    pub lista_no_local_padrao: bool,
    /// 📍 Os locais cujo rótulo a tela não mostra a esta conta.
    pub locais_ocultos: Vec<crate::local::LocalDaSessao>,
    /// 📍 Os caixas que a conta pode ter abertos em cada estúdio.
    pub caixas: Vec<crate::local::LocalDoCaixa>,
    /// 📍 Em qual desses caixas cai a venda da sessão de cada local.
    pub caixa_de_cada_local: crate::local::CaixaDeCadaLocal,
}

impl Acesso {
    pub fn pode(&self, permissao: &str) -> bool {
        self.admin || self.permissoes.iter().any(|p| p == permissao)
    }

    /// O perfil é de **alguns** estúdios: a conta não vê os outros, nem as
    /// sessões sem estúdio.
    pub fn estudios_restritos(&self) -> bool {
        !self.admin && self.estudios.is_some()
    }

    /// Opera o balcão e não tem estúdio nenhum liberado: entra e não vê nada.
    /// É quando a tela troca a lista vazia pelo aviso de procurar quem
    /// administra o sistema.
    pub fn sem_estudio(&self) -> bool {
        self.estudios_restritos() && self.estudios.as_ref().is_some_and(Vec::is_empty)
    }

    /// 📍 Esta conta vê o rótulo deste local?
    pub fn mostra_local(&self, local: crate::local::LocalDaSessao) -> bool {
        !self.locais_ocultos.contains(&local)
    }

    /// A linha do menu da conta: `Balcão de Gramado` / `Administrador`.
    pub fn rotulo(&self) -> Option<String> {
        if self.admin {
            return Some("Administrador".into());
        }
        let perfil = self.perfil.as_deref()?;
        Some(if self.perfil_ativo {
            perfil.to_string()
        } else {
            format!("{perfil} (desativado)")
        })
    }
}

/// 🔐 **O estúdio único** da conta: o perfil de acesso dela é de alguns
/// estúdios, e só um chegou na lista. Aí não há o que escolher — nem a
/// pergunta da entrada, nem seletor: fica esse (dono, 11/out/2026: *"se o
/// usuário tiver acesso somente a um estúdio não pode aparecer tela de seleção
/// e nem combo de seleção"*). É o `estudioUnico` do site.
///
/// Administrador e perfil de "todos os estúdios" **não** entram, mesmo com um
/// estúdio só no cadastro: para eles "todos" ainda mostra a sessão sem estúdio.
pub fn estudio_unico<E>(estudios: &[E], estudios_restritos: bool) -> Option<&E> {
    match estudios {
        [unico] if estudios_restritos => Some(unico),
        _ => None,
    }
}

/// A conta pode isto? Sem `acesso` (API de antes dos perfis), pode: quem
/// recusa é o backend.
pub fn pode(acesso: Option<&Acesso>, permissao: &str) -> bool {
    acesso.is_none_or(|a| a.pode(permissao))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn do_balcao(estudios: Option<&[&str]>) -> Acesso {
        Acesso {
            admin: false,
            perfil: Some("Balcão de Gramado".into()),
            perfil_ativo: true,
            permissoes: vec![OPERAR_O_BALCAO.into()],
            estudios: estudios.map(|e| e.iter().map(|s| s.to_string()).collect()),
            ..Acesso::default()
        }
    }

    #[test]
    fn o_administrador_pode_tudo_e_nao_tem_estudio_restrito() {
        let admin = Acesso {
            admin: true,
            ..Acesso::default()
        };
        assert!(admin.pode(CAIXA) && admin.pode(AGENDA) && admin.pode(ACERVO));
        assert!(!admin.estudios_restritos());
        assert_eq!(admin.rotulo().as_deref(), Some("Administrador"));
    }

    #[test]
    fn o_perfil_so_pode_o_que_marca() {
        let acesso = do_balcao(Some(&["gramado"]));
        assert!(acesso.pode(OPERAR_O_BALCAO));
        assert!(!acesso.pode(CAIXA));
        assert!(!acesso.pode(CONVERSAS));
        assert!(acesso.estudios_restritos());
        assert!(!acesso.sem_estudio());
        assert_eq!(acesso.rotulo().as_deref(), Some("Balcão de Gramado"));
    }

    #[test]
    fn perfil_de_todos_os_estudios_nao_e_restrito() {
        let acesso = do_balcao(None);
        assert!(!acesso.estudios_restritos());
        assert!(!acesso.sem_estudio());
    }

    #[test]
    fn perfil_sem_estudio_liberado_pede_o_aviso() {
        assert!(do_balcao(Some(&[])).sem_estudio());
    }

    #[test]
    fn perfil_desativado_diz_que_esta_desativado() {
        let acesso = Acesso {
            perfil_ativo: false,
            permissoes: vec![],
            ..do_balcao(Some(&[]))
        };
        assert_eq!(
            acesso.rotulo().as_deref(),
            Some("Balcão de Gramado (desativado)")
        );
    }

    #[test]
    fn so_o_perfil_de_um_estudio_fixa_o_estudio() {
        assert_eq!(estudio_unico(&["gramado"], true), Some(&"gramado"));
        // Dois no perfil: a escolha continua.
        assert_eq!(estudio_unico(&["gramado", "canela"], true), None);
        // Administrador ou perfil de todos, com um só no cadastro: escolhe.
        assert_eq!(estudio_unico(&["gramado"], false), None);
        assert_eq!(estudio_unico::<&str>(&[], true), None);
    }

    /// A API de antes dos perfis não manda `acesso`: nada some da tela.
    #[test]
    fn sem_acesso_na_resposta_nada_se_esconde() {
        assert!(pode(None, CAIXA));
        assert!(!pode(Some(&do_balcao(None)), CAIXA));
    }
}

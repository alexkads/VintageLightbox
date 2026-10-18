//! 📍 O **local** da sessão fotográfica — no estúdio ou fora dele.
//!
//! É o `local` das galerias do site (dono, 18/out/2026: *"um recurso na sessão
//! de fotos chamado local, onde pode ser ESTÚDIO ou EXTERNO"*), espelho de
//! `domain::pos_venda::local` do `recordarfotos-e-commerce`. O perfil de acesso
//! decide, por local, o caixa separado, o rótulo escondido, o chatbot e o modo
//! stealth — quem aplica é o backend; aqui só o que a tela precisa.
//!
//! 🔒 **Mudança interna**: nenhum texto que o cliente leia, e nenhuma novidade
//! da versão, cita o local.

/// Onde a sessão foi feita. O padrão é o estúdio — o de toda sessão até
/// 18/out/2026.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LocalDaSessao {
    #[default]
    Estudio,
    Externo,
}

impl LocalDaSessao {
    pub const TODOS: [LocalDaSessao; 2] = [LocalDaSessao::Estudio, LocalDaSessao::Externo];

    /// O texto da API — o mesmo do `CHECK` do banco.
    pub fn como_texto(self) -> &'static str {
        match self {
            Self::Estudio => "ESTUDIO",
            Self::Externo => "EXTERNO",
        }
    }

    /// Estrito: texto desconhecido é `None`, nunca o vizinho mais próximo.
    pub fn ler(texto: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|l| l.como_texto() == texto)
    }

    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Estudio => "Estúdio",
            Self::Externo => "Externo",
        }
    }
}

/// O caixa geral — o de quem não separa um local — na API.
pub const CAIXA_GERAL: &str = "GERAL";

/// Qual dos caixas do operador num estúdio: o de um local, ou o geral (`None`).
pub type LocalDoCaixa = Option<LocalDaSessao>;

/// O texto do caixa na API: `ESTUDIO`, `EXTERNO` ou `GERAL`.
pub fn texto_do_caixa(local: LocalDoCaixa) -> &'static str {
    local.map_or(CAIXA_GERAL, LocalDaSessao::como_texto)
}

/// O caixa como a API o escreve. Desconhecido é `None` (não se sabe), e não o
/// geral: confundir os dois mandaria a venda para outra gaveta.
pub fn ler_caixa(texto: &str) -> Option<LocalDoCaixa> {
    if texto == CAIXA_GERAL {
        return Some(None);
    }
    LocalDaSessao::ler(texto).map(Some)
}

pub fn rotulo_do_caixa(local: LocalDoCaixa) -> &'static str {
    local.map_or("Geral", LocalDaSessao::rotulo)
}

/// O nome do caixa na tela: "Caixa Externo", "Caixa Geral" — ou só "Caixa"
/// quando a conta tem um caixa só, ou quando o local é um que o perfil não
/// mostra. O `nomeDoCaixa` do site.
pub fn nome_do_caixa(
    local: LocalDoCaixa,
    caixas: &[LocalDoCaixa],
    ocultos: &[LocalDaSessao],
) -> String {
    if caixas.len() <= 1 || local.is_some_and(|l| ocultos.contains(&l)) {
        return "Caixa".into();
    }
    format!("Caixa {}", rotulo_do_caixa(local))
}

/// Em qual caixa cai a venda da sessão de cada local — o `caixa_de_cada_local`
/// do `/auth/me`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaixaDeCadaLocal {
    pub estudio: LocalDoCaixa,
    pub externo: LocalDoCaixa,
}

impl Default for CaixaDeCadaLocal {
    /// Cada local no seu — o padrão do perfil e de quem não tem perfil.
    fn default() -> Self {
        Self {
            estudio: Some(LocalDaSessao::Estudio),
            externo: Some(LocalDaSessao::Externo),
        }
    }
}

impl CaixaDeCadaLocal {
    pub fn da_sessao(&self, local: LocalDaSessao) -> LocalDoCaixa {
        match local {
            LocalDaSessao::Estudio => self.estudio,
            LocalDaSessao::Externo => self.externo,
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_texto_do_local_volta_igual_e_o_desconhecido_nao_vira_vizinho() {
        for l in LocalDaSessao::TODOS {
            assert_eq!(LocalDaSessao::ler(l.como_texto()), Some(l));
        }
        assert_eq!(LocalDaSessao::ler("estudio"), None);
        assert_eq!(ler_caixa("GERAL"), Some(None));
        assert_eq!(ler_caixa("EXTERNO"), Some(Some(LocalDaSessao::Externo)));
        assert_eq!(ler_caixa("outro"), None);
    }

    #[test]
    fn o_nome_do_caixa_so_diz_o_local_quando_ha_mais_de_um() {
        let dois = [Some(LocalDaSessao::Estudio), Some(LocalDaSessao::Externo)];
        assert_eq!(
            nome_do_caixa(Some(LocalDaSessao::Externo), &dois, &[]),
            "Caixa Externo"
        );
        assert_eq!(
            nome_do_caixa(None, &[Some(LocalDaSessao::Estudio), None], &[]),
            "Caixa Geral"
        );
        assert_eq!(
            nome_do_caixa(Some(LocalDaSessao::Estudio), &dois[..1], &[]),
            "Caixa"
        );
        assert_eq!(
            nome_do_caixa(
                Some(LocalDaSessao::Externo),
                &dois,
                &[LocalDaSessao::Externo]
            ),
            "Caixa"
        );
    }

    #[test]
    fn a_sessao_de_cada_local_cai_no_caixa_do_mapa() {
        let mapa = CaixaDeCadaLocal {
            estudio: Some(LocalDaSessao::Estudio),
            externo: None,
        };
        assert_eq!(mapa.da_sessao(LocalDaSessao::Externo), None);
        assert_eq!(
            CaixaDeCadaLocal::default().da_sessao(LocalDaSessao::Externo),
            Some(LocalDaSessao::Externo)
        );
    }
}

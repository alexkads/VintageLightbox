//! O que a bandeja diz, linha por linha.
//!
//! ⚠️ **Não há "Conexão: nova tentativa em N s" nem "Enviar agora", e é de
//! propósito**: aqui cada pedido sai uma vez e a falha vira recusa com motivo.
//! Mostrar um relógio de nova tentativa que não existe seria mentir.

use crate::menu::NOME;

/// Tudo o que a bandeja mostra, tirado da raiz a cada volta do laço.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Retrato {
    /// Pedidos ao site ainda sem resposta (subir, tirar, salvar revelação).
    pub subindo: usize,
    /// Fotos do ensaio **rejeitadas** (a tecla `X`): elas ficam no disco e
    /// **não sobem** — contrato C21.
    ///
    /// 🔄 **Era `sem_nota`, "esperando nota"**, e a linha dizia a verdade até
    /// 2026-09-20: sem nota a foto não subia. Com C20 o ensaio inteiro sobe em
    /// segundo plano, e quem fica esperando alguma coisa é só a rejeitada —
    /// esperando o operador mudar de ideia.
    pub rejeitadas: usize,
    /// O que o site recusou nesta abertura.
    pub recusadas: usize,
    /// Miniaturas sendo refeitas do disco.
    pub refazendo: usize,
    pub conta: Option<String>,
    pub pilha_local: bool,
    /// Segundos unix da última resposta boa do site.
    pub ultimo_envio: Option<i64>,
    pub bytes_do_catalogo: Option<u64>,
    /// Agora, em segundos unix: "há 2 min" é relativo a ele.
    pub agora: i64,
}

impl Retrato {
    /// Há trabalho que fechar a janela interromperia (G9).
    ///
    /// 🔑 **Só o envio conta.** Miniatura refeita é cache: some com o app e
    /// volta na próxima abertura, sem perda nenhuma.
    pub fn ha_envio_pendente(&self) -> bool {
        self.subindo > 0
    }
}

/// As linhas da bandeja, uma por item do menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Linhas {
    pub cabecalho: String,
    pub conta: String,
    pub subindo: String,
    pub rejeitadas: String,
    pub recusadas: String,
    pub segundo_plano: String,
    pub ultimo: String,
    pub catalogo: String,
    /// A dica do ícone: o resumo numa linha.
    pub dica: String,
}

pub fn plural(n: usize, um: &str, varios: &str) -> String {
    if n == 1 {
        format!("1 {um}")
    } else {
        format!("{n} {varios}")
    }
}

/// "agora há pouco", "há 5 min", "há 3 h", "há 2 dias".
pub fn ha_quanto(segundos: i64) -> String {
    match segundos.max(0) {
        0..=59 => "agora há pouco".into(),
        s @ 60..=3599 => format!("há {} min", s / 60),
        s @ 3600..=86_399 => format!("há {} h", s / 3600),
        s => format!("há {}", plural((s / 86_400) as usize, "dia", "dias")),
    }
}

/// "12 KB", "3,4 MB", "1,2 GB".
pub fn tamanho(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    let (valor, unidade) = if b < KB * KB {
        (b / KB, "KB")
    } else if b < KB * KB * KB {
        (b / (KB * KB), "MB")
    } else {
        (b / (KB * KB * KB), "GB")
    };
    let texto = if valor >= 100.0 || unidade == "KB" {
        format!("{valor:.0}")
    } else {
        format!("{valor:.1}")
    };
    format!("{} {unidade}", texto.replace('.', ","))
}

/// O que cada linha diz, pelo retrato.
pub fn linhas(r: &Retrato) -> Linhas {
    let versao = env!("CARGO_PKG_VERSION");
    let cabecalho = if r.pilha_local {
        format!("{NOME} {versao} · PILHA LOCAL")
    } else {
        format!("{NOME} {versao}")
    };
    let conta = match &r.conta {
        Some(email) => format!("Conta: {email}"),
        None => "Conta: sem sessão".into(),
    };
    let subindo = match r.subindo {
        0 => "Subindo: nada na fila".into(),
        n => format!("Subindo: {}", plural(n, "foto", "fotos")),
    };
    let rejeitadas = match r.rejeitadas {
        0 => "Rejeitadas (não sobem): nenhuma".into(),
        n => format!("Rejeitadas (não sobem): {}", plural(n, "foto", "fotos")),
    };
    let recusadas = match r.recusadas {
        0 => "Recusadas pelo servidor: nenhuma".into(),
        n => format!("Recusadas pelo servidor: {n} (veja no app)"),
    };
    // A frase da barra da revelação na web ("Em segundo plano N").
    let segundo_plano = match r.refazendo {
        0 => "Em segundo plano: nada".into(),
        n => format!("Em segundo plano: {}", plural(n, "miniatura", "miniaturas")),
    };
    let ultimo = match r.ultimo_envio {
        Some(quando) => format!("Último envio: {}", ha_quanto(r.agora - quando)),
        None => "Último envio: nenhum ainda".into(),
    };
    let catalogo = match r.bytes_do_catalogo {
        Some(b) => format!("Catálogo neste computador: {}", tamanho(b)),
        None => "Catálogo neste computador: calculando…".into(),
    };

    let mut resumo = Vec::new();
    if r.subindo > 0 {
        resumo.push(format!("{} subindo", plural(r.subindo, "foto", "fotos")));
    }
    if r.recusadas > 0 {
        resumo.push(format!(
            "{} pelo servidor",
            plural(r.recusadas, "recusada", "recusadas")
        ));
    }
    if r.rejeitadas > 0 {
        resumo.push(format!("{} rejeitadas", r.rejeitadas));
    }
    let dica = if resumo.is_empty() {
        format!("{NOME} — Tudo sincronizado")
    } else {
        format!("{NOME} — {}", resumo.join(" · "))
    };

    Linhas {
        cabecalho,
        conta,
        subindo,
        rejeitadas,
        recusadas,
        segundo_plano,
        ultimo,
        catalogo,
        dica,
    }
}

/// O tamanho de uma pasta, somando os arquivos (sem seguir atalhos).
pub fn tamanho_da_pasta(pasta: &std::path::Path) -> u64 {
    let mut total = 0;
    let mut pendentes = vec![pasta.to_path_buf()];
    while let Some(atual) = pendentes.pop() {
        let Ok(entradas) = std::fs::read_dir(&atual) else {
            continue;
        };
        for entrada in entradas.flatten() {
            let Ok(tipo) = entrada.file_type() else {
                continue;
            };
            if tipo.is_dir() {
                pendentes.push(entrada.path());
            } else if tipo.is_file() {
                total += entrada.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    total
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn sem_nada_pendente_diz_que_esta_tudo_certo() {
        let l = linhas(&Retrato {
            agora: 1_000_000,
            ..Default::default()
        });
        assert_eq!(l.conta, "Conta: sem sessão");
        assert_eq!(l.subindo, "Subindo: nada na fila");
        assert_eq!(l.rejeitadas, "Rejeitadas (não sobem): nenhuma");
        assert_eq!(l.recusadas, "Recusadas pelo servidor: nenhuma");
        assert_eq!(l.segundo_plano, "Em segundo plano: nada");
        assert_eq!(l.ultimo, "Último envio: nenhum ainda");
        assert_eq!(l.catalogo, "Catálogo neste computador: calculando…");
        assert_eq!(l.dica, format!("{NOME} — Tudo sincronizado"));
        assert!(l.cabecalho.starts_with(NOME));
        assert!(!l.cabecalho.contains("PILHA LOCAL"));
    }

    #[test]
    fn cada_linha_conta_a_sua_parte() {
        let r = Retrato {
            subindo: 5,
            rejeitadas: 3,
            recusadas: 1,
            refazendo: 1,
            conta: Some("dono@estudio".into()),
            pilha_local: true,
            ultimo_envio: Some(1_000_000 - 300),
            bytes_do_catalogo: Some(3 * 1024 * 1024 + 512 * 1024),
            agora: 1_000_000,
        };
        let l = linhas(&r);
        assert!(l.cabecalho.ends_with("· PILHA LOCAL"), "{}", l.cabecalho);
        assert_eq!(l.conta, "Conta: dono@estudio");
        assert_eq!(l.subindo, "Subindo: 5 fotos");
        assert_eq!(l.rejeitadas, "Rejeitadas (não sobem): 3 fotos");
        assert_eq!(l.recusadas, "Recusadas pelo servidor: 1 (veja no app)");
        assert_eq!(l.segundo_plano, "Em segundo plano: 1 miniatura");
        assert_eq!(l.ultimo, "Último envio: há 5 min");
        assert_eq!(l.catalogo, "Catálogo neste computador: 3,5 MB");
        assert_eq!(
            l.dica,
            format!("{NOME} — 5 fotos subindo · 1 recusada pelo servidor · 3 rejeitadas")
        );
    }

    #[test]
    fn uma_foto_so_fala_no_singular() {
        let l = linhas(&Retrato {
            subindo: 1,
            rejeitadas: 1,
            refazendo: 7,
            ..Default::default()
        });
        assert_eq!(l.subindo, "Subindo: 1 foto");
        assert_eq!(l.rejeitadas, "Rejeitadas (não sobem): 1 foto");
        assert_eq!(l.segundo_plano, "Em segundo plano: 7 miniaturas");
        assert_eq!(
            l.dica,
            format!("{NOME} — 1 foto subindo · 1 rejeitadas"),
            "miniatura refeita não entra no resumo: é cache, não envio"
        );
    }

    #[test]
    fn so_o_envio_segura_o_app_aberto() {
        let refazendo = Retrato {
            refazendo: 30,
            rejeitadas: 4,
            recusadas: 2,
            ..Default::default()
        };
        assert!(!refazendo.ha_envio_pendente());
        let subindo = Retrato {
            subindo: 1,
            ..Default::default()
        };
        assert!(subindo.ha_envio_pendente());
    }

    #[test]
    fn o_tempo_e_o_tamanho_em_palavras() {
        assert_eq!(ha_quanto(-5), "agora há pouco", "relógio adiantado");
        assert_eq!(ha_quanto(10), "agora há pouco");
        assert_eq!(ha_quanto(7200), "há 2 h");
        assert_eq!(ha_quanto(86_400), "há 1 dia");
        assert_eq!(ha_quanto(3 * 86_400), "há 3 dias");
        assert_eq!(tamanho(2048), "2 KB");
        assert_eq!(tamanho(5 * 1024 * 1024 * 1024 / 2), "2,5 GB");
        assert_eq!(tamanho(150 * 1024 * 1024), "150 MB");
    }

    #[test]
    fn o_tamanho_da_pasta_soma_os_arquivos() {
        let pasta = tempfile::tempdir().unwrap();
        std::fs::write(pasta.path().join("a"), [0u8; 10]).unwrap();
        std::fs::create_dir(pasta.path().join("b")).unwrap();
        std::fs::write(pasta.path().join("b/c"), [0u8; 5]).unwrap();
        assert_eq!(tamanho_da_pasta(pasta.path()), 15);
        assert_eq!(tamanho_da_pasta(&pasta.path().join("nao-existe")), 0);
    }
}

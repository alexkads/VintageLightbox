//! O catálogo do app Tauri: o SQLite e as pastas das fotos (DESKTOP_TAURI §0,
//! D13, etapa D).
//!
//! Este módulo cuida do primeiro passo da etapa D, **abrir com segurança**:
//!
//! - **G1, um dono por catálogo.** Uma trava de arquivo (`.trava`) segura o
//!   catálogo enquanto o app está aberto. Uma segunda instância recebe
//!   [`ErroDoCatalogo::EmUso`], e o plugin de instância única, em `main.rs`,
//!   traz a janela da primeira para frente antes disso.
//! - **G2, esquema versionado.** As migrations de `migracoes/` vão embutidas no
//!   binário, numeradas. Ao abrir, as que faltam são aplicadas uma a uma, cada
//!   uma numa transação que também grava a versão (`PRAGMA user_version`). Antes
//!   da primeira, o banco é copiado para `catalogo.db.antes-v<N>`. Uma que falha
//!   **impede o app de abrir**, com a mensagem dela, em vez de rodar sobre um
//!   esquema pela metade. Um catálogo mais novo que o app também é recusado:
//!   abrir com um binário velho seria gravar sem conhecer as regras novas.

use std::fs::File;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

/// As migrations, na ordem. A versão do esquema é o tamanho desta lista.
///
/// ⚠️ Só se acrescenta no fim. Uma migration publicada nunca muda nem sai.
const MIGRACOES: &[(&str, &str)] = &[
    ("0001_catalogo", include_str!("migracoes/0001_catalogo.sql")),
    (
        "0002_espera_da_fila",
        include_str!("migracoes/0002_espera_da_fila.sql"),
    ),
    (
        "0003_area_temporaria",
        include_str!("migracoes/0003_area_temporaria.sql"),
    ),
];

pub const ARQUIVO_DO_BANCO: &str = "catalogo.db";
const ARQUIVO_DA_TRAVA: &str = ".trava";
/// As pastas do catálogo. `caches/` pode ser apagada a qualquer hora (C14).
/// `envios/` guarda os arquivos da fila até o servidor confirmar.
pub const PASTAS: &[&str] = &["fotos", "reveladas", "caches", "envios", importacao::PASTA];

#[derive(Debug, thiserror::Error)]
pub enum ErroDoCatalogo {
    #[error("não foi possível criar o catálogo em {0}: {1}")]
    Pasta(PathBuf, std::io::Error),
    #[error("o catálogo em {0} já está aberto em outra janela do VintageLightbox")]
    EmUso(PathBuf),
    #[error("não foi possível abrir o banco do catálogo: {0}")]
    Banco(#[from] rusqlite::Error),
    #[error(
        "o catálogo está na versão {encontrada}, mais nova que a deste app ({conhecida}). \
         Atualize o VintageLightbox antes de abri-lo."
    )]
    VersaoMaisNova { encontrada: usize, conhecida: usize },
    #[error("não foi possível guardar a cópia do catálogo antes de atualizá-lo: {0}")]
    Copia(rusqlite::Error),
    #[error("{0}")]
    Entrada(String),
    #[error("a atualização {nome} do catálogo falhou, e nada foi alterado: {erro}")]
    Migracao {
        nome: &'static str,
        erro: rusqlite::Error,
    },
}

/// O catálogo aberto. A trava vale enquanto este valor existir.
pub struct Catalogo {
    raiz: PathBuf,
    banco: Connection,
    _trava: File,
}

/// O que a tela e o diagnóstico mostram do catálogo.
#[derive(Debug, serde::Serialize)]
pub struct Situacao {
    pub caminho: String,
    pub versao: usize,
}

impl Catalogo {
    /// Abre (ou cria) o catálogo em `raiz`.
    pub fn abrir(raiz: &Path) -> Result<Self, ErroDoCatalogo> {
        for pasta in PASTAS {
            std::fs::create_dir_all(raiz.join(pasta))
                .map_err(|e| ErroDoCatalogo::Pasta(raiz.to_path_buf(), e))?;
        }

        // G1: a trava vem antes do banco. Duas instâncias nunca chegam a
        // migrar o mesmo arquivo ao mesmo tempo.
        let trava = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(raiz.join(ARQUIVO_DA_TRAVA))
            .map_err(|e| ErroDoCatalogo::Pasta(raiz.to_path_buf(), e))?;
        if trava.try_lock().is_err() {
            return Err(ErroDoCatalogo::EmUso(raiz.to_path_buf()));
        }

        let caminho = raiz.join(ARQUIVO_DO_BANCO);
        let mut banco = Connection::open(&caminho)?;
        banco.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )?;
        migrar(&mut banco, &caminho)?;

        Ok(Catalogo {
            raiz: raiz.to_path_buf(),
            banco,
            _trava: trava,
        })
    }

    pub fn situacao(&self) -> Result<Situacao, ErroDoCatalogo> {
        Ok(Situacao {
            caminho: self.raiz.to_string_lossy().into_owned(),
            versao: versao(&self.banco)?,
        })
    }

    /// O banco, para os testes.
    #[cfg(test)]
    pub fn banco(&self) -> &Connection {
        &self.banco
    }
}

fn versao(banco: &Connection) -> Result<usize, rusqlite::Error> {
    banco
        .query_row("PRAGMA user_version", [], |linha| linha.get::<_, i64>(0))
        .map(|v| v as usize)
}

/// G2: aplica as migrations que faltam, cada uma numa transação.
fn migrar(banco: &mut Connection, caminho: &Path) -> Result<(), ErroDoCatalogo> {
    migrar_com(banco, caminho, MIGRACOES)
}

fn migrar_com(
    banco: &mut Connection,
    caminho: &Path,
    migracoes: &[(&'static str, &str)],
) -> Result<(), ErroDoCatalogo> {
    let atual = versao(banco)?;
    let conhecida = migracoes.len();
    if atual > conhecida {
        return Err(ErroDoCatalogo::VersaoMaisNova {
            encontrada: atual,
            conhecida,
        });
    }
    if atual == conhecida {
        return Ok(());
    }

    // Um banco novo não tem o que copiar. Um que já tem dados ganha a cópia
    // antes de mudar. `VACUUM INTO` escreve um arquivo consistente mesmo com o
    // WAL ligado, o que um `cp` não garante.
    if atual > 0 {
        let copia = caminho.with_extension(format!("db.antes-v{}", atual + 1));
        let _ = std::fs::remove_file(&copia);
        banco
            .execute("VACUUM INTO ?1", [copia.to_string_lossy()])
            .map_err(ErroDoCatalogo::Copia)?;
    }

    for (indice, (nome, sql)) in migracoes.iter().enumerate().skip(atual) {
        let transacao = banco.transaction()?;
        transacao
            .execute_batch(sql)
            .and_then(|_| transacao.pragma_update(None, "user_version", (indice + 1) as i64))
            .map_err(|erro| ErroDoCatalogo::Migracao { nome, erro })?;
        transacao
            .commit()
            .map_err(|erro| ErroDoCatalogo::Migracao { nome, erro })?;
    }
    Ok(())
}

pub mod fila;
pub mod importacao;

#[cfg(test)]
mod testes;

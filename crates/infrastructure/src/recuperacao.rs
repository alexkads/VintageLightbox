//! Recuperar um cartão formatado de verdade: abrir o dispositivo, varrer e
//! gravar cada foto achada no destino.
//!
//! Roda **num processo à parte, com privilégio de administrador** (o
//! `ui-gpui --recuperar`), porque ler o cartão setor por setor exige isso nos
//! três sistemas. A janela não é elevada e não conversa com ele por stdout: no
//! Windows, o processo aberto pelo UAC não devolve saída a quem o chamou. O
//! canal é o próprio destino:
//!
//! - [`ARQUIVO_DE_ANDAMENTO`]: este processo grava o [`AndamentoGravado`] ali,
//!   sempre trocando o arquivo inteiro (grava ao lado e renomeia), para a
//!   janela nunca ler meio JSON;
//! - [`ARQUIVO_DE_PARADA`]: a janela cria o arquivo quando o operador aperta
//!   "Parar", e a varredura termina no próximo bloco.
//!
//! 🚨 **O dispositivo é aberto só para leitura.** Nada deste módulo grava no
//! cartão; o destino ser outro disco é conferido antes, no
//! `use_cases::recuperacao::conferir_destino`.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use domain::recuperacao::nome_da_recuperada;
use recuperacao_core::{copiar, varrer, Leitor};
use serde::{Deserialize, Serialize};

pub const ARQUIVO_DE_ANDAMENTO: &str = ".recuperacao.json";
pub const ARQUIVO_DE_PARADA: &str = ".recuperacao.parar";

/// De quanto em quanto o andamento é regravado.
const INTERVALO: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AndamentoGravado {
    pub lidos: u64,
    pub total: u64,
    pub achadas: usize,
    /// Blocos que o cartão não deixou ler.
    pub ilegiveis: u32,
    pub terminou: bool,
    pub interrompida: bool,
    pub erro: Option<String>,
}

pub fn ler_andamento(destino: &Path) -> Option<AndamentoGravado> {
    let texto = fs::read_to_string(destino.join(ARQUIVO_DE_ANDAMENTO)).ok()?;
    serde_json::from_str(&texto).ok()
}

fn gravar_andamento(destino: &Path, andamento: &AndamentoGravado) {
    let provisorio = destino.join(format!("{ARQUIVO_DE_ANDAMENTO}.tmp"));
    let Ok(texto) = serde_json::to_string(andamento) else {
        return;
    };
    if fs::write(&provisorio, texto).is_ok() {
        dar_ao_dono_do_destino(&provisorio, destino);
        let _ = fs::rename(&provisorio, destino.join(ARQUIVO_DE_ANDAMENTO));
    }
}

/// O processo roda como root, e o que ele grava nasceria de root: o operador
/// não conseguiria nem apagar as fotos que recuperou. Cada arquivo passa a ser
/// de quem é dono da pasta de destino, que foi a janela (o operador) que criou.
#[cfg(unix)]
fn dar_ao_dono_do_destino(arquivo: &Path, destino: &Path) {
    use std::os::unix::fs::MetadataExt;
    if let Ok(meta) = fs::metadata(destino) {
        let _ = std::os::unix::fs::chown(arquivo, Some(meta.uid()), Some(meta.gid()));
    }
}

#[cfg(not(unix))]
fn dar_ao_dono_do_destino(_arquivo: &Path, _destino: &Path) {}

/// O primeiro nome livre a partir de `n`. Recuperar duas vezes para a mesma
/// pasta não sobrescreve o que a primeira gravou.
fn caminho_livre(destino: &Path, n: &mut usize, extensao: &str) -> PathBuf {
    loop {
        let caminho = destino.join(nome_da_recuperada(*n, extensao));
        if !caminho.exists() {
            return caminho;
        }
        *n += 1;
    }
}

/// Varre `dispositivo` e grava em `destino` cada foto achada.
///
/// `tamanho` é o do cartão, da listagem; zero é "descubra" (serve à imagem
/// `.img` e ao Linux, onde o fim do dispositivo se acha pelo `seek`).
pub fn recuperar(
    dispositivo: &Path,
    destino: &Path,
    tamanho: u64,
) -> io::Result<recuperacao_core::Resumo> {
    let _ = fs::remove_file(destino.join(ARQUIVO_DE_PARADA));
    let mut andamento = AndamentoGravado::default();

    let resultado = (|| {
        let mut arquivo = File::open(dispositivo)?;
        let total = if tamanho > 0 {
            Some(tamanho)
        } else {
            use std::io::Seek;
            match arquivo.seek(io::SeekFrom::End(0)) {
                Ok(0) | Err(_) => None,
                Ok(t) => Some(t),
            }
        };
        andamento.total = total.unwrap_or(0);
        gravar_andamento(destino, &andamento);

        let mut leitor = Leitor::novo(arquivo, total);
        let parada = destino.join(ARQUIVO_DE_PARADA);
        let mut ultima = Instant::now();
        let mut n = 1usize;
        // O andamento é gravado pelos dois fechos; a célula evita dois `&mut`.
        let estado = std::cell::RefCell::new(&mut andamento);

        let resumo = varrer(
            &mut leitor,
            &|| parada.exists(),
            &mut |a| {
                let mut e = estado.borrow_mut();
                e.lidos = a.lidos;
                e.achadas = a.achadas;
                if ultima.elapsed() >= INTERVALO {
                    gravar_andamento(destino, &e);
                    ultima = Instant::now();
                }
            },
            &mut |leitor, achado| {
                let final_ = caminho_livre(destino, &mut n, achado.formato.extensao());
                n += 1;
                // Grava ao lado e renomeia: uma foto pela metade (disco cheio,
                // cartão arrancado) nunca aparece com o nome de uma inteira.
                let parcial = final_.with_extension("parcial");
                let gravou = (|| {
                    let mut saida = io::BufWriter::new(File::create(&parcial)?);
                    copiar(leitor, achado, &mut saida)?;
                    io::Write::flush(&mut saida)?;
                    fs::rename(&parcial, &final_)
                })();
                if let Err(e) = gravou {
                    let _ = fs::remove_file(&parcial);
                    return Err(e);
                }
                dar_ao_dono_do_destino(&final_, destino);
                Ok(())
            },
        )?;
        Ok(resumo)
    })();

    match &resultado {
        Ok(resumo) => {
            andamento.lidos = resumo.lidos;
            andamento.achadas = resumo.achadas;
            andamento.ilegiveis = resumo.ilegiveis;
            andamento.interrompida = resumo.interrompida;
        }
        Err(e) => andamento.erro = Some(explicar(e, dispositivo)),
    }
    andamento.terminou = true;
    gravar_andamento(destino, &andamento);
    let _ = fs::remove_file(destino.join(ARQUIVO_DE_PARADA));
    resultado
}

fn explicar(erro: &io::Error, dispositivo: &Path) -> String {
    match erro.kind() {
        io::ErrorKind::PermissionDenied => format!(
            "Sem permissão para ler {}. A recuperação precisa da senha de administrador.",
            dispositivo.display()
        ),
        io::ErrorKind::NotFound => format!(
            "{} não existe mais. O cartão saiu do leitor?",
            dispositivo.display()
        ),
        _ => erro.to_string(),
    }
}

/// A entrada do modo `--recuperar <dispositivo> <destino> [tamanho]` do
/// binário. Devolve o código de saída do processo.
pub fn rodar_pela_linha_de_comando(args: &[String]) -> i32 {
    let posicao = args.iter().position(|a| a == "--recuperar");
    let Some(p) = posicao else {
        return 2;
    };
    let (Some(dispositivo), Some(destino)) = (args.get(p + 1), args.get(p + 2)) else {
        eprintln!("uso: --recuperar <dispositivo> <destino> [tamanho]");
        return 2;
    };
    let tamanho = args.get(p + 3).and_then(|t| t.parse().ok()).unwrap_or(0u64);
    let destino = Path::new(destino);
    if let Err(e) = fs::create_dir_all(destino) {
        eprintln!("não deu para criar {}: {e}", destino.display());
        return 1;
    }
    match recuperar(Path::new(dispositivo), destino, tamanho) {
        Ok(resumo) => {
            println!("{} fotos recuperadas", resumo.achadas);
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Um JPEG mínimo com a estrutura inteira (SOI, DQT, SOF0, SOS, dados,
    /// EOI), do tamanho pedido.
    fn jpeg(semente: u8, dados: usize) -> Vec<u8> {
        let seg = |m: u8, corpo: &[u8]| {
            let mut s = vec![0xFF, m];
            s.extend(((corpo.len() + 2) as u16).to_be_bytes());
            s.extend(corpo);
            s
        };
        let mut j = vec![0xFF, 0xD8];
        j.extend(seg(0xE0, b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0"));
        j.extend(seg(0xDB, &[semente; 65]));
        j.extend(seg(0xC0, &[8, 0, 8, 0, 8, 1, 1, 0x11, 0]));
        j.extend(seg(0xDA, &[1, 1, 0, 0, 63, 0]));
        j.extend((0..dados).map(|i| ((i as u8) ^ semente) & 0x7F));
        j.extend([0xFF, 0xD9]);
        j
    }

    #[test]
    fn recupera_para_a_pasta_com_andamento_e_sem_sobrescrever() {
        let dir = tempfile::tempdir().unwrap();
        let fotos = [jpeg(1, 10_000), jpeg(2, 70_000)];
        let mut cartao = vec![0u8; 64 * 1024];
        for f in &fotos {
            cartao.extend(f);
            cartao.resize(cartao.len().div_ceil(32 * 1024) * 32 * 1024, 0);
        }
        let img = dir.path().join("cartao.img");
        fs::write(&img, &cartao).unwrap();
        let destino = dir.path().join("recuperadas");
        fs::create_dir_all(&destino).unwrap();

        let resumo = recuperar(&img, &destino, 0).unwrap();
        assert_eq!(resumo.achadas, 2);
        assert_eq!(
            fs::read(destino.join("recuperada-00001.jpg")).unwrap(),
            fotos[0]
        );
        assert_eq!(
            fs::read(destino.join("recuperada-00002.jpg")).unwrap(),
            fotos[1]
        );
        let andamento = ler_andamento(&destino).unwrap();
        assert!(andamento.terminou);
        assert_eq!(andamento.achadas, 2);
        assert_eq!(andamento.lidos, cartao.len() as u64);
        assert_eq!(andamento.erro, None);

        // A segunda vez não pisa na primeira.
        recuperar(&img, &destino, 0).unwrap();
        assert_eq!(
            fs::read(destino.join("recuperada-00003.jpg")).unwrap(),
            fotos[0]
        );
    }

    #[test]
    fn uma_parada_velha_nao_impede_de_comecar() {
        let dir = tempfile::tempdir().unwrap();
        let img = dir.path().join("cartao.img");
        fs::write(&img, vec![0u8; 1 << 20]).unwrap();
        // Uma parada que sobrou de uma recuperação anterior faria a nova
        // terminar antes do primeiro bloco. `recuperar` apaga a velha ao
        // começar; parar **durante** é o `varredura::testes::parar_interrompe`.
        fs::write(dir.path().join(ARQUIVO_DE_PARADA), b"").unwrap();
        let resumo = recuperar(&img, dir.path(), 0).unwrap();
        assert!(!resumo.interrompida);
        assert!(!dir.path().join(ARQUIVO_DE_PARADA).exists());
    }

    #[test]
    fn dispositivo_que_nao_existe_vira_erro_no_andamento() {
        let dir = tempfile::tempdir().unwrap();
        assert!(recuperar(&dir.path().join("sumiu"), dir.path(), 0).is_err());
        let andamento = ler_andamento(dir.path()).unwrap();
        assert!(andamento.terminou);
        assert!(andamento.erro.unwrap().contains("saiu do leitor"));
    }
}

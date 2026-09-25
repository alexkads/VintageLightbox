//! 🏋️ **Carga: a triagem com o estúdio inteiro trabalhando por baixo.**
//!
//! O cenário do dono (25/set/2026): *"as vezes a importação é longa e a
//! medida que o sistema for processando as fotos eu preciso exibir para o
//! cliente, e qualquer processo em segundo plano do tipo enviar para a
//! Cloudflare R2 não pode bloquear"*. Tudo ao mesmo tempo, no app inteiro:
//!
//! - **uma importação longa**, foto a foto, num catálogo que já tem milhares;
//! - **o ensaio subindo ao R2 com a rede cheia** — cada envio e cada `PATCH`
//!   fica preso até o site responder, uma resposta por vez;
//! - **o operador classificando sem parar**, nas fotos do site e nas que
//!   acabaram de entrar do cartão.
//!
//! O que o teste cobra:
//!
//! 1. **nenhuma tecla se perde** — a nota de cada gesto chega ao site ou ao
//!    catálogo, e é a última apertada que vale;
//! 2. **a nota aparece na hora**, antes de qualquer resposta;
//! 3. **a foto importada aparece enquanto o lote anda**, e recebe nota;
//! 4. **quanto a tela fica presa** — cobrado só no `make carga` (otimizado,
//!    sozinho, `VLB_CARGA=1`); na suíte comum é só impresso:
//!    - a tecla, tratada pelo app, cabe folgada num quadro (é um dos blocos
//!      abaixo); com o redesenho completo do simulador, até dois (p95);
//!    - **nenhum bloco contínuo** de trabalho passa de um quadro — é o que
//!      vira soluço na tela (medido trecho a trecho pela [`crate::regua`]);
//!    - mesmo no pico, o trabalho de fundo deixa a thread livre **pelo menos
//!      metade do tempo**.
//!
//! ⚠️ **Por que não "cada volta cabe num quadro"**: o executor de teste roda
//! em sequência, sem pausa, tudo o que 150 ms de relógio acumularam. No app
//! esse trabalho se espalha entre os quadros; somá-lo mede ocupação, e não
//! travamento. Foi o primeiro critério desta carga, e reprovava um app que
//! nunca prendia a tela por mais de 4 ms.
//!
//! ```bash
//! make carga
//! ```

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use domain::services::pos_venda::EstadoDaFotoNoSite;
use gpui::TestAppContext;

use super::{abrir_o_ensaio, do_site, local, Cenario};

/// Quantas fotos o cartão traz.
///
/// ⚠️ **Em `debug` a escala é menor**: lá o teste só confere que nada se
/// perde (o tempo não vale nada sem otimização), e não pode pesar no
/// `make testar`. A carga de verdade é `make carga`.
const IMPORTADAS: usize = if cfg!(debug_assertions) { 60 } else { 300 };
/// Quantas o ensaio já tem no site.
const DO_SITE: usize = if cfg!(debug_assertions) { 30 } else { 150 };
/// Quantas o catálogo tem de outros ensaios — o peso de cada releitura.
const OUTRAS_NO_CATALOGO: usize = if cfg!(debug_assertions) { 300 } else { 3_000 };
/// Quantas voltas do relógio uma foto importada tem para aparecer na grade:
/// o respiro da releitura (450 ms) mais a espera dela (100 ms), com folga.
const VOLTAS_PARA_APARECER: usize = 6;

/// Uma volta do relógio: o tempo que as colheitas e releituras acumulam.
const VOLTA: Duration = Duration::from_millis(150);

/// Um quadro a 60 fps: a tecla tem de caber aqui para parecer instantânea.
const UM_QUADRO: Duration = Duration::from_micros(16_700);

#[gpui::test]
fn importacao_longa_com_o_r2_lento_nao_trava_a_triagem(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(
        cx,
        Cenario {
            site: Box::new(|site| {
                // 🐢 A rede cheia: envio ao R2 e `PATCH` só voltam quando o
                // teste soltar, um por vez.
                site.demorada = true;
                site.negociacao_demorada = true;
                site.fotos_da_sessao
                    .lock()
                    .unwrap()
                    .extend((0..DO_SITE).map(|i| {
                        do_site(
                            &format!("s{i:03}"),
                            10 + i as i32,
                            EstadoDaFotoNoSite::Disponivel,
                            None,
                        )
                    }));
            }),
            importador_demorado: true,
            ..Cenario::default()
        },
    );
    {
        let mut fotos = e.acervo.fotos.lock().unwrap();
        for i in 0..OUTRAS_NO_CATALOGO {
            let mut foto = local(&format!("OUTRA_{i:05}.jpg"));
            foto.sessao_id = Some("outro-ensaio".into());
            fotos.push(foto);
        }
    }
    e.esperar(cx);

    // 📥 O lote sai.
    let caminhos: Vec<String> = (0..IMPORTADAS)
        .map(|i| format!("/cartao/IMP_{i:04}.jpg"))
        .collect();
    e.detalhe(cx, |tela, _w, cx| tela.enviar_arquivos(caminhos, cx));
    e.esperar(cx);
    e.importador.responder_uma(); // o `Comecou`
    let no_inicio = e.detalhe(cx, |tela, _w, _cx| tela.total_visivel());

    crate::regua::zerar();
    let mut esperado: BTreeMap<String, u8> = BTreeMap::new();
    let mut teclas: Vec<Duration> = Vec::with_capacity(IMPORTADAS);
    let mut voltas: Vec<Duration> = Vec::with_capacity(IMPORTADAS);

    for passo in 0..IMPORTADAS {
        // 💾 O disco: a foto entra no catálogo e o importador conta.
        e.acervo
            .fotos
            .lock()
            .unwrap()
            .push(local(&format!("IMP_{passo:04}.jpg")));
        e.importador.responder_uma();

        // ☁️ O R2: a cada duas voltas, **uma** resposta sai da fila.
        if passo % 2 == 0 {
            e.site.responder_uma();
        }

        // 👆 O operador: uma foto, uma nota. Uma em cada três é das que
        // acabaram de chegar do cartão — é ela que prova que a importada
        // aparece a tempo de ser mostrada ao cliente.
        let alvo = if passo % 3 == 0 && passo >= VOLTAS_PARA_APARECER {
            format!("id-IMP_{:04}.jpg", passo - VOLTAS_PARA_APARECER)
        } else {
            format!("s{:03}", (passo * 7) % DO_SITE)
        };
        let nota = (passo % 5 + 1) as u8;
        let focada = e.detalhe(cx, |tela, _w, cx| {
            tela.focar_foto(&alvo, cx);
            tela.em_foco().map(|f| f.id.clone())
        });
        assert_eq!(
            focada.as_deref(),
            Some(alvo.as_str()),
            "passo {passo}: {alvo} não estava na grade para receber a nota"
        );

        let inicio = Instant::now();
        e.teclar(cx, &nota.to_string());
        teclas.push(inicio.elapsed());

        let na_grade = e.detalhe(cx, |tela, _w, _cx| tela.em_foco().and_then(|f| f.nota));
        assert_eq!(
            na_grade,
            Some(nota),
            "passo {passo}: a nota {nota} em {alvo} não apareceu na hora"
        );
        esperado.insert(alvo, nota);

        // ⏱️ O relógio anda: colheitas, releituras, a esteira.
        let inicio = Instant::now();
        cx.executor().advance_clock(VOLTA);
        cx.run_until_parked();
        voltas.push(inicio.elapsed());

        if passo == IMPORTADAS / 2 {
            let (agora, importando) = e.detalhe(cx, |tela, _w, _cx| {
                (tela.total_visivel(), tela.importando())
            });
            assert!(importando, "o lote ainda está no meio");
            assert!(
                agora >= no_inicio + IMPORTADAS / 2 - VOLTAS_PARA_APARECER,
                "no meio do lote só {} das {} importadas estavam na grade",
                agora - no_inicio,
                passo + 1
            );
        }
    }

    // ⏱️ Onde a tela ficou presa durante os passos — antes do esvaziamento.
    let onde = crate::regua::relatorio();
    let (trecho_pior, bloco_pior) = crate::regua::pior().expect("a régua mediu alguma coisa");

    // 🏁 O cartão acaba e a rede esvazia.
    for _ in 0..4 {
        e.importador.responder_uma();
    }
    // ⚠️ A fila do site nunca zera: a galeria viva pergunta sozinha, de
    // tempos em tempos. O que tem de acabar é **o trabalho**: envio e `PATCH`.
    let trabalho_na_fila = |e: &super::Estudio| {
        e.site.guardados.lock().unwrap().iter().any(|(_, recado)| {
            matches!(
                recado,
                crate::pos_venda::porta::Recado::Negociou { .. }
                    | crate::pos_venda::porta::Recado::ClassificadaSubiu { .. }
            )
        })
    };
    // A esteira manda o próximo envio a cada resposta: esvazia até o
    // trabalho acabar, e não por um número fixo de voltas.
    for _ in 0..(IMPORTADAS * 2) {
        if !trabalho_na_fila(&e) {
            break;
        }
        e.site.responder();
        e.esperar(cx);
    }
    e.esperar(cx);
    assert!(
        !trabalho_na_fila(&e),
        "sobrou envio ou PATCH preso na fila do site"
    );

    // 1. Nenhuma tecla se perdeu, e vale a última.
    let no_site: BTreeMap<String, Option<u8>> = e
        .site
        .fotos_da_sessao
        .lock()
        .unwrap()
        .iter()
        .map(|f| (f.id.clone(), f.nota))
        .collect();
    let no_catalogo: BTreeMap<String, i32> = e
        .acervo
        .fotos
        .lock()
        .unwrap()
        .iter()
        .map(|f| (f.id.clone(), f.rating))
        .collect();
    let mut perdidas = Vec::new();
    for (id, nota) in &esperado {
        let gravada = if id.starts_with("id-IMP_") {
            no_catalogo.get(id).map(|r| *r as u8)
        } else {
            no_site.get(id).copied().flatten()
        };
        if gravada != Some(*nota) {
            perdidas.push(format!("{id}: pedida {nota}, gravada {gravada:?}"));
        }
    }
    assert!(
        perdidas.is_empty(),
        "{} de {} notas não chegaram: {perdidas:?}",
        perdidas.len(),
        esperado.len()
    );

    // 2. E a grade, depois de tudo assentar, mostra o que foi gravado.
    let na_grade: Vec<(String, Option<u8>)> = e.detalhe(cx, |tela, _w, cx| {
        esperado
            .keys()
            .map(|id| {
                tela.focar_foto(id, cx);
                (id.clone(), tela.em_foco().and_then(|f| f.nota))
            })
            .collect()
    });
    for (id, nota) in na_grade {
        assert_eq!(
            nota,
            esperado.get(&id).copied(),
            "a grade desfez a nota de {id}"
        );
    }

    // 3. Quanto a tela ficou presa.
    let medir = |mut tempos: Vec<Duration>| {
        tempos.sort();
        let em = |q: f64| tempos[((tempos.len() - 1) as f64 * q) as usize];
        (em(0.5), em(0.95), tempos[tempos.len() - 1])
    };
    let (tecla_mediana, tecla_p95, tecla_pior) = medir(teclas);
    let (volta_mediana, volta_p95, volta_pior) = medir(voltas);
    println!(
        "🏋️ carga: {IMPORTADAS} importadas, {DO_SITE} no site, {OUTRAS_NO_CATALOGO} no catálogo, \
         {} notas\n   tecla: mediana {tecla_mediana:?}, p95 {tecla_p95:?}, pior {tecla_pior:?}\n   \
         volta do relógio (150 ms): mediana {volta_mediana:?}, p95 {volta_p95:?}, pior {volta_pior:?}\n   \
         maior bloco contínuo: {bloco_pior:?} em \"{trecho_pior}\"\n\
         {onde}",
        esperado.len()
    );
    // ⚠️ **Tempo só vale com a máquina para ele**: otimizado (`make carga`,
    // que liga `VLB_CARGA`) e sozinho numa thread. Na suíte, com quatro
    // testes pesados dividindo os núcleos, a mesma tecla chegou a 50 ms — e
    // isso mede a disputa, não o app. Lá o teste cobra só a correção acima.
    let medindo_de_verdade = !cfg!(debug_assertions) && std::env::var_os("VLB_CARGA").is_some();
    if medindo_de_verdade {
        // 🔑 **O custo do app por tecla é o bloco contínuo abaixo** (a régua
        // mede `dar_nota` por dentro: 0,4 ms no pior caso, 25/set/2026). A
        // volta inteira do simulador inclui o redesenho completo que o teste
        // força, e oscila entre rodadas iguais (15 a 19,5 ms): cobrar um
        // quadro dela reprovaria pelo ruído da máquina. Ela fica como rede
        // contra regressão grande.
        assert!(
            tecla_p95 <= 2 * UM_QUADRO,
            "a tecla, com o redesenho, passou de dois quadros em 5% das vezes: p95 {tecla_p95:?}"
        );
        assert!(
            tecla_pior <= 3 * UM_QUADRO,
            "uma tecla, com o redesenho, levou {tecla_pior:?} — mais de três quadros"
        );
        assert!(
            bloco_pior <= UM_QUADRO,
            "\"{trecho_pior}\" prendeu a tela por {bloco_pior:?} de uma vez — um quadro perdido"
        );
        assert!(
            volta_p95 <= VOLTA / 2,
            "no pico o trabalho de fundo ocupou {volta_p95:?} de cada {VOLTA:?}: a tela ficou livre \
             menos da metade do tempo"
        );
    }
}

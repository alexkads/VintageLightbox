//! 📤 **A esteira de envios** — o que na web é o Worker, aqui é este módulo.
//!
//! # Por que existe (dono, 18/set/2026)
//!
//! > *"Essas atividades de 'Sincronizar' e 'Salvar na galeria e sair' precisam
//! > ser realmente em segundo plano, da mesma forma que ocorre no `app-tauri` e
//! > no site, pois eu sei que eles usam Service Worker. Precisa criar algum
//! > módulo no Rust que faça esse mesmo trabalho paralelo em fila!"*
//!
//! No site, quem sobe as fotos é um Worker: a tela entrega a lista e continua
//! respondendo ao operador, e o Worker leva **três por vez**
//! (`editor.tsx`, `EM_VOO`). Aqui era o contrário — cada gesto disparava uma
//! tarefa por foto na mesma passada, e dois defeitos saíram disso no mesmo dia:
//!
//! - **a memória** (*"a aplicação estoura a memória na hora de sair e salvar em
//!   segundo plano"*): cada foto no ar é um original decodificado, 96 MB em
//!   RAM. Duzentas de uma vez levam a máquina do balcão junto — e o que chega ao
//!   servidor quando a alocação falha não é imagem, daí o `400: formato de
//!   imagem não suportado` que uma sessão de 200 fotos devolveu;
//! - **a tela** (*"o usuário não consegue operar a biblioteca durante a
//!   atualização das fotos"*): sem fila, o lote inteiro competia com a interface
//!   pela GPU e pelo disco.
//!
//! # O que este módulo é, e o que não é
//!
//! É **uma fila com teto**, e nada mais: quem executa continua sendo a porta do
//! pós-venda (`Publicador`), que já roda no tokio. A esteira decide *quando*
//! cada trabalho sai — e é isso que a torna testável sem rede, sem GPU e sem
//! janela.
//!
//! ⚠️ **Ela não guarda nada em disco.** O que protege o trabalho contra um
//! fechamento no meio é o depósito do gravador (a receita) e a nota no catálogo
//! — a fila é só a ordem em que as coisas saem nesta sessão do app. É a mesma
//! divisão da web: o Worker não é a memória, o IndexedDB é.

use domain::services::pos_venda::Sessao;
use std::collections::VecDeque;
use std::sync::mpsc::Sender;

use crate::pos_venda::porta::{FotoClassificada, Publicador, Recado};
use crate::revelacao::processador::Ajustes;
use domain::value_objects::CropSettings;

/// Quantos trabalhos a esteira mantém **no ar** ao mesmo tempo.
///
/// 🔑 **Três, e o número é o do site** (`editor.tsx`, `EM_VOO`). Cada foto é
/// baixar (rede), decodificar e revelar (96 MB e a GPU, serializados pela fila
/// do motor) e subir (rede). Com uma só, a GPU fica parada durante os segundos
/// de rede de cada foto; com várias, o download de uma corre enquanto outra
/// revela e uma terceira sobe.
///
/// ⚠️ **O teto é memória, não banda.** As que esperam a vez seguram o original
/// comprimido — uns 30 MB cada. Três em voo é ~90 MB parados mais os 96 MB da
/// que está sendo decodificada; subir isso para dez leva a máquina do balcão
/// junto, que é onde este botão roda.
pub const EM_VOO: usize = 3;

/// Um trabalho da esteira.
///
/// 🔑 **Os dois caminhos que sobem foto**, e só eles: classificar (o passo 3) e
/// salvar a revelação (o botão do editor). O que não sobe arquivo — negociar,
/// mudar faixa, tirar do site — continua indo direto: é um `PATCH` de alguns
/// bytes, sem imagem e sem memória.
#[derive(Debug, Clone)]
pub enum Trabalho {
    /// O passo 3: a foto classificada sobe para a galeria.
    Classificada {
        galeria: String,
        foto: Box<FotoClassificada>,
    },
    /// O "Salvar na galeria e sair": a revelação entra no lugar do original.
    ///
    /// ⚠️ `Box` nos dois lados: `Ajustes` são 171 `f32`, e uma variante gorda
    /// faria toda a fila carregar o tamanho da maior.
    Revelacao {
        foto_no_site: String,
        ajustes: Box<Ajustes>,
        corte: CropSettings,
    },
}

impl Trabalho {
    /// O que este trabalho mexe — é por aqui que a esteira não repete a mesma
    /// foto duas vezes na fila.
    fn alvo(&self) -> &str {
        match self {
            Self::Classificada { foto, .. } => &foto.foto_id,
            Self::Revelacao { foto_no_site, .. } => foto_no_site,
        }
    }
}

/// Quanto da esteira já andou — o que a bandeja e o canto dos envios mostram.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Progresso {
    /// Quantos trabalhos entraram desde que a esteira esvaziou pela última vez.
    pub total: usize,
    /// Quantos já responderam (com sucesso ou falha).
    pub respondidos: usize,
    /// Quantos estão no ar agora.
    pub no_ar: usize,
    /// Alguma resposta veio como falha? O aviso do fim depende disso.
    pub houve_falha: bool,
}

impl Progresso {
    pub fn andando(&self) -> bool {
        self.respondidos < self.total
    }

    /// Quantos ainda esperam vaga.
    pub fn na_fila(&self) -> usize {
        self.total
            .saturating_sub(self.respondidos)
            .saturating_sub(self.no_ar)
    }
}

/// A fila com teto.
#[derive(Debug, Default)]
pub struct Esteira {
    fila: VecDeque<Trabalho>,
    progresso: Progresso,
}

impl Esteira {
    /// Põe um trabalho na fila.
    ///
    /// 🚨 **A mesma foto não entra duas vezes na fila**: classificar em lote e
    /// repetir o gesto antes de a primeira resposta voltar mandaria a mesma foto
    /// ao site duas vezes — e a segunda desfaria a primeira depois de dois
    /// downloads e dois JPEGs. É a mesma regra de `enfileirar_para_subir`.
    ///
    /// ⚠️ **O que já está no ar não é comparado**: ele já saiu, e a resposta
    /// dele vem de qualquer jeito. Quem repete um gesto sobre uma foto que
    /// acabou de sair está pedindo a segunda versão dela, e é o que acontece.
    pub fn empurrar(&mut self, trabalho: Trabalho) {
        if self.fila.iter().any(|t| t.alvo() == trabalho.alvo()) {
            return;
        }
        self.fila.push_back(trabalho);
        self.progresso.total += 1;
    }

    /// Manda para a porta os próximos, até [`EM_VOO`] no ar. Devolve quantos
    /// saíram agora.
    pub fn despachar(
        &mut self,
        publicador: &dyn Publicador,
        sessao: &Sessao,
        canal: &Sender<Recado>,
    ) -> usize {
        let mut saíram = 0;
        while self.progresso.no_ar < EM_VOO {
            let Some(trabalho) = self.fila.pop_front() else {
                break;
            };
            match trabalho {
                Trabalho::Classificada { galeria, foto } => {
                    publicador.subir_classificada(sessao.clone(), galeria, *foto, canal.clone());
                }
                Trabalho::Revelacao {
                    foto_no_site,
                    ajustes,
                    corte,
                } => {
                    publicador.salvar_revelacao(
                        sessao.clone(),
                        foto_no_site,
                        *ajustes,
                        corte,
                        canal.clone(),
                    );
                }
            }
            self.progresso.no_ar += 1;
            saíram += 1;
        }
        saíram
    }

    /// Uma resposta voltou: abre uma vaga.
    ///
    /// 🔑 **Quem reabastece é a resposta**, e não o relógio: a esteira não sabe
    /// quanto uma foto demora, e um temporizador acabaria mandando mais fotos
    /// enquanto as primeiras ainda decodificam — que é exatamente o que ela
    /// existe para impedir.
    pub fn uma_respondeu(&mut self, falhou: bool) {
        if self.progresso.no_ar == 0 && self.progresso.respondidos >= self.progresso.total {
            // Um eco atrasado (recado que ninguém pediu) não faz a conta dar a
            // volta — o mesmo cuidado do contador de sincronias.
            return;
        }
        self.progresso.no_ar = self.progresso.no_ar.saturating_sub(1);
        self.progresso.respondidos += 1;
        self.progresso.houve_falha |= falhou;
        if !self.progresso.andando() && self.fila.is_empty() {
            // Esteira vazia: a contagem recomeça no próximo lote.
            self.progresso = Progresso::default();
        }
    }

    pub fn progresso(&self) -> Progresso {
        self.progresso
    }

    /// Há o que fazer? (Na fila ou no ar.)
    pub fn andando(&self) -> bool {
        !self.fila.is_empty() || self.progresso.no_ar > 0
    }

    /// Esvazia o que ainda não saiu — o que está no ar continua, porque já saiu.
    pub fn esquecer_o_que_espera(&mut self) {
        self.progresso.total -= self.fila.len();
        self.fila.clear();
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::pos_venda::porta::mentira::PublicadorDeMentira;
    use std::sync::mpsc::channel;
    use std::sync::Arc;

    fn sessao() -> Sessao {
        Sessao {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            access_vence_em: i64::MAX,
            refresh_vence_em: i64::MAX,
        }
    }

    fn classificada(id: &str) -> Trabalho {
        Trabalho::Classificada {
            galeria: "g1".into(),
            foto: Box::new(FotoClassificada {
                foto_id: id.into(),
                ordem: 0,
                estado: None,
                nota: Some(5),
                produto_id: None,
            }),
        }
    }

    /// 🚨 **Três no ar, nunca mais** — o teto é a razão de a esteira existir.
    #[test]
    fn a_esteira_segura_o_lote_em_tres() {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let (canal, _recebe) = channel();
        let mut esteira = Esteira::default();
        for i in 0..10 {
            esteira.empurrar(classificada(&format!("f{i}")));
        }

        let saíram = esteira.despachar(publicador.as_ref(), &sessao(), &canal);

        assert_eq!(saíram, EM_VOO);
        assert_eq!(publicador.subidas().len(), EM_VOO);
        assert_eq!(esteira.progresso().no_ar, EM_VOO);
        assert_eq!(esteira.progresso().na_fila(), 7);

        // Sem resposta, um segundo despacho não manda nada: as vagas estão
        // ocupadas.
        assert_eq!(esteira.despachar(publicador.as_ref(), &sessao(), &canal), 0);
        assert_eq!(publicador.subidas().len(), EM_VOO);
    }

    /// Cada resposta abre uma vaga — e o lote inteiro sai, em ondas de três.
    #[test]
    fn cada_resposta_abre_uma_vaga() {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let (canal, _recebe) = channel();
        let mut esteira = Esteira::default();
        for i in 0..10 {
            esteira.empurrar(classificada(&format!("f{i}")));
        }
        esteira.despachar(publicador.as_ref(), &sessao(), &canal);

        for _ in 0..10 {
            esteira.uma_respondeu(false);
            esteira.despachar(publicador.as_ref(), &sessao(), &canal);
        }

        assert_eq!(publicador.subidas().len(), 10, "todas subiram");
        assert!(!esteira.andando(), "e a esteira esvaziou");
        assert_eq!(
            esteira.progresso(),
            Progresso::default(),
            "a contagem recomeça no próximo lote"
        );
    }

    /// 🚨 A mesma foto não entra duas vezes na fila.
    #[test]
    fn a_mesma_foto_nao_entra_duas_vezes() {
        let mut esteira = Esteira::default();
        esteira.empurrar(classificada("a"));
        esteira.empurrar(classificada("a"));
        esteira.empurrar(classificada("b"));

        assert_eq!(esteira.progresso().total, 2);
    }

    /// A falha conta como resposta — e fica registrada para o aviso do fim.
    #[test]
    fn a_falha_conta_e_fica_registrada() {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let (canal, _recebe) = channel();
        let mut esteira = Esteira::default();
        esteira.empurrar(classificada("a"));
        esteira.empurrar(classificada("b"));
        esteira.despachar(publicador.as_ref(), &sessao(), &canal);

        esteira.uma_respondeu(true);
        assert!(esteira.progresso().houve_falha);
        assert_eq!(esteira.progresso().no_ar, 1);

        esteira.uma_respondeu(false);
        assert!(!esteira.andando());
    }

    /// ⚠️ Um eco atrasado não faz a conta dar a volta.
    #[test]
    fn resposta_que_ninguem_pediu_nao_estraga_a_conta() {
        let mut esteira = Esteira::default();

        esteira.uma_respondeu(false);
        esteira.uma_respondeu(true);

        assert_eq!(esteira.progresso(), Progresso::default());
        assert!(!esteira.progresso().houve_falha);
    }

    /// O que ainda não saiu pode ser esquecido; o que está no ar, não.
    #[test]
    fn esquecer_o_que_espera_nao_cancela_o_que_ja_saiu() {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let (canal, _recebe) = channel();
        let mut esteira = Esteira::default();
        for i in 0..8 {
            esteira.empurrar(classificada(&format!("f{i}")));
        }
        esteira.despachar(publicador.as_ref(), &sessao(), &canal);

        esteira.esquecer_o_que_espera();

        assert_eq!(esteira.progresso().no_ar, EM_VOO, "as três continuam");
        assert_eq!(esteira.progresso().total, EM_VOO);
        assert_eq!(esteira.progresso().na_fila(), 0);
    }
}

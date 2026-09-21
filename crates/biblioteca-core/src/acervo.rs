//! O que a biblioteca mostra, o que ela filtra e o que ela deixa mudar.
//!
//! # Por que o acervo está no core
//!
//! A rota `/dashboard/sessoes-fotograficas/{id}` não é uma grade: é uma grade
//! **mais** o filtro por situação com contagem, a régua de "o que ainda pode
//! mudar" e o que a seleção permite fazer em lote. Essas três regras estavam em
//! TypeScript, dentro do componente, e são exatamente as que o VintageLightbox
//! precisa repetir para ter a mesma tela — repetir é como se erra.
//!
//! 🔑 **O core decide, quem chama executa.** Aqui não há Server Action, nem
//! banco, nem HTTP: ele responde *quais* fotos, *quantas* e *se pode*. Gravar
//! continua sendo do site (Server Actions) e do desktop (casos de uso), cada um
//! com a sua transação.

/// A situação de uma foto no balcão — os três valores do `CREATE TYPE` do
/// pós-venda, e nada além deles.
///
/// ⚠️ **Nunca inventar um quarto na conversão.** Valor desconhecido vindo do
/// hospedeiro é erro, e não "o vizinho mais próximo" (armadilha nº 6 do
/// projeto): tratar `comprada` como `disponivel` por engano poria à venda o que
/// já foi vendido.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Estado {
    LevadaNoBalcao,
    Disponivel,
    Comprada,
}

impl Estado {
    pub fn como_texto(self) -> &'static str {
        match self {
            Estado::LevadaNoBalcao => "levada_no_balcao",
            Estado::Disponivel => "disponivel",
            Estado::Comprada => "comprada",
        }
    }

    /// `None` para valor desconhecido — ver o aviso do enum.
    pub fn do_texto(texto: &str) -> Option<Estado> {
        match texto {
            "levada_no_balcao" => Some(Estado::LevadaNoBalcao),
            "disponivel" => Some(Estado::Disponivel),
            "comprada" => Some(Estado::Comprada),
            _ => None,
        }
    }

    /// Como a tela chama isto em português.
    pub fn rotulo(self) -> &'static str {
        match self {
            Estado::LevadaNoBalcao => "Levada",
            Estado::Disponivel => "À venda",
            Estado::Comprada => "Comprada",
        }
    }
}

/// Uma foto como a biblioteca precisa conhecê-la — o suficiente para filtrar,
/// contar, permitir e desenhar. O resto (URLs de prévia, datas formatadas,
/// ajustes da revelação) fica com quem desenha.
#[derive(Debug, Clone, PartialEq)]
pub struct Foto {
    pub id: String,
    pub arquivo: String,
    pub estado: Estado,
    /// A retenção apagou o arquivo. A linha continua; a foto não volta.
    pub apagada: bool,
    /// A faixa que vale para esta foto (a dela, ou a da galeria).
    pub produto_efetivo: String,
    pub preco_negociado: Option<i64>,
    pub tem_observacao: bool,
    /// Preço fixado para a compra online, em centavos. `None` = vale a faixa.
    pub preco_de_venda: Option<i64>,
    pub pedido_id: Option<String>,
    pub downloads: u32,
    pub revelada: bool,
    /// A nota de 1 a 5 do fotógrafo. `None` = **não classificada**.
    ///
    /// 🚨 **Sem nota, a foto não devia estar aqui** (regra do dono,
    /// 2026-09-05): só sobe para o acervo o que foi classificado. Mas `None`
    /// existe e continua existindo, por duas razões: as fotos que já estavam na
    /// galeria quando a coluna nasceu, e o recorte que as encontra — que é
    /// justamente para o operador as classificar ou tirar.
    pub nota: Option<u8>,
    /// A foto foi **rejeitada** — a tecla `X` (contrato C21).
    ///
    /// 🚨 Rejeitar **marca**: a foto some da galeria do cliente, deixa de ser
    /// comprável e não vai ao balcão — e continua inteira no acervo. Nada é
    /// apagado por este campo.
    ///
    /// ⚠️ Distinto de `nota == None`, que é "ainda não passou pela curadoria" e
    /// sobe e vende normalmente (C20, C22).
    pub rejeitada: bool,
    pub ordem: i64,
}

impl Foto {
    /// 🔑 **Só o que o operador ainda pode mudar: nem comprada, nem apagada.**
    ///
    /// A comprada tem cobrança atrás dela — mudar a faixa mudaria o preço do que
    /// já foi pago. A apagada não tem arquivo: não há o que vender, revelar nem
    /// entregar.
    pub fn editavel(&self) -> bool {
        self.estado != Estado::Comprada && !self.apagada
    }

    /// Houve conversa de balcão sobre esta foto (cortesia, desconto, já paga).
    pub fn tem_negociacao(&self) -> bool {
        self.preco_negociado.is_some() || self.tem_observacao
    }
}

/// O recorte que a barra oferece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filtro {
    Todas,
    Situacao(Estado),
    Apagadas,
    /// As que ninguém classificou — o recorte que o dono pediu em 2026-09-05.
    ///
    /// 🔄 Até 2026-09-21 ela era um problema a resolver (não ia à venda nem ao
    /// storage). Hoje está à venda e sobe com marca d'água; o recorte continua
    /// porque vender **no balcão** pede nota, e achá-las numa galeria de
    /// duzentas sem ele é olhar foto por foto.
    SemNota,
    /// As que **têm nota** — o oposto de [`Filtro::SemNota`].
    ///
    /// Serve à pergunta que o operador faz no fim do atendimento: *"o que já
    /// está classificado?"*. Sem ele, a resposta é somar três chips de cabeça.
    Classificadas,
    /// As que o operador **rejeitou** com a tecla `X` (contrato C21).
    ///
    /// 🔑 Elas saem de todos os outros recortes menos "Todas": não estão à
    /// venda, não vão ao balcão e não esperam curadoria — a decisão já foi
    /// tomada. É aqui que se acham para desfazer, com o mesmo `X`.
    Rejeitadas,
}

impl Filtro {
    /// ⚠️ **A apagada só aparece no recorte dela.** Ela não tem arquivo, e
    /// misturá-la com "à venda" ofereceria ao cliente uma foto que não existe
    /// mais — por isso todo recorte por situação exige `!apagada`.
    pub fn bate(self, foto: &Foto) -> bool {
        match self {
            Filtro::Todas => true,
            Filtro::Apagadas => foto.apagada,
            // 🚨 **Sem classificação não é "à venda" nem "levada".** A regra
            // do dono (2026-09-05) é que a foto só pode ficar à venda se
            // estiver classificada; contá-la no recorte de venda dizia o
            // contrário na primeira linha da tela — *"à venda 8"* numa galeria
            // em que nenhuma das oito tinha nota, e nenhuma sequer havia
            // subido. Elas moram no recorte `SemNota`, que existe para isso.
            //
            // 🔄 **Revogado pelo dono em 2026-09-21**: *"A foto com
            // classificação 0 (zero) fica disponível para venda e irá para
            // nuvem com marca d'água; para a nova regra, somente as fotos
            // rejeitadas não serão enviadas para nuvem"*. A sem nota está à
            // venda como qualquer outra; a venda **no balcão** continua pedindo
            // nota e `P`, e quem cobra isso é o balcão, não o recorte.
            //
            // 🚨 **A rejeitada sai** (C21): ela não está à venda em lugar
            // nenhum.
            Filtro::Situacao(estado) => !foto.apagada && !foto.rejeitada && foto.estado == estado,
            // Rejeitada não é "sem nota" (C21): a sem nota espera curadoria, a
            // rejeitada já foi decidida.
            Filtro::SemNota => !foto.apagada && !foto.rejeitada && foto.nota.is_none(),
            Filtro::Classificadas => !foto.apagada && !foto.rejeitada && foto.nota.is_some(),
            Filtro::Rejeitadas => !foto.apagada && foto.rejeitada,
        }
    }
}

/// Quantas fotos há em cada recorte — o número ao lado de cada botão da barra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Contagens {
    pub todas: usize,
    pub levadas: usize,
    pub a_venda: usize,
    pub compradas: usize,
    pub apagadas: usize,
    /// Quantas estão sem classificação — as do recorte `SemNota`.
    pub sem_nota: usize,
    /// Quantas têm nota.
    pub classificadas: usize,
    /// Quantas foram rejeitadas (`X`).
    pub rejeitadas: usize,
}

impl Contagens {
    pub fn de(self, filtro: Filtro) -> usize {
        match filtro {
            Filtro::Todas => self.todas,
            Filtro::Situacao(Estado::LevadaNoBalcao) => self.levadas,
            Filtro::Situacao(Estado::Disponivel) => self.a_venda,
            Filtro::Situacao(Estado::Comprada) => self.compradas,
            Filtro::Apagadas => self.apagadas,
            Filtro::SemNota => self.sem_nota,
            Filtro::Classificadas => self.classificadas,
            Filtro::Rejeitadas => self.rejeitadas,
        }
    }
}

/// O acervo da galeria e o recorte em vigor.
#[derive(Debug, Default)]
pub struct Acervo {
    fotos: Vec<Foto>,
    filtro: Option<FiltroEmVigor>,
    visiveis: Vec<usize>,
}

#[derive(Debug, Clone, Copy)]
struct FiltroEmVigor(Filtro);

impl Default for FiltroEmVigor {
    fn default() -> Self {
        Self(Filtro::Todas)
    }
}

impl Acervo {
    pub fn novo() -> Self {
        Self::default()
    }

    /// Troca o acervo inteiro — é o que a página faz a cada revalidação.
    pub fn definir(&mut self, fotos: Vec<Foto>) {
        self.fotos = fotos;
        self.recalcular();
    }

    pub fn filtrar(&mut self, filtro: Filtro) {
        self.filtro = Some(FiltroEmVigor(filtro));
        self.recalcular();
    }

    pub fn filtro(&self) -> Filtro {
        self.filtro.unwrap_or_default().0
    }

    fn recalcular(&mut self) {
        let filtro = self.filtro();
        self.visiveis = self
            .fotos
            .iter()
            .enumerate()
            .filter(|(_, f)| filtro.bate(f))
            .map(|(i, _)| i)
            .collect();
    }

    pub fn todas(&self) -> &[Foto] {
        &self.fotos
    }

    /// Quantas o recorte mostra — é o `total` que a grade desenha.
    pub fn total_visivel(&self) -> usize {
        self.visiveis.len()
    }

    /// A foto na posição `n` da grade (já filtrada).
    pub fn visivel(&self, n: usize) -> Option<&Foto> {
        self.visiveis.get(n).and_then(|i| self.fotos.get(*i))
    }

    pub fn visiveis(&self) -> impl Iterator<Item = &Foto> {
        self.visiveis.iter().filter_map(|i| self.fotos.get(*i))
    }

    /// Onde esta foto está na grade — `None` se o recorte a escondeu.
    pub fn posicao_de(&self, id: &str) -> Option<usize> {
        self.visiveis.iter().position(|i| self.fotos[*i].id == id)
    }

    /// 🚨 As contagens são do **acervo inteiro**, não do recorte.
    ///
    /// É o que os botões da barra mostram, e é o que faz "À venda 4" continuar
    /// dizendo 4 enquanto se olha "Compradas". Contar sobre o recorte daria o
    /// número da tela em vez do número da galeria — a armadilha nº 4 do projeto
    /// (número parcial que se apresenta como total).
    pub fn contagens(&self) -> Contagens {
        let mut c = Contagens {
            todas: self.fotos.len(),
            ..Default::default()
        };
        // 🔑 **Contado pelo próprio `Filtro::bate`**, recorte a recorte: o
        // número que a barra mostra é o que o recorte devolve, sempre. Contar
        // com uma segunda regra aqui foi o que deixou a D21 passar — as duas
        // listas da mesma verdade.
        for f in &self.fotos {
            let conta = |filtro: Filtro, n: &mut usize| {
                if filtro.bate(f) {
                    *n += 1;
                }
            };
            conta(Filtro::Apagadas, &mut c.apagadas);
            conta(Filtro::SemNota, &mut c.sem_nota);
            conta(Filtro::Classificadas, &mut c.classificadas);
            conta(Filtro::Rejeitadas, &mut c.rejeitadas);
            conta(Filtro::Situacao(Estado::LevadaNoBalcao), &mut c.levadas);
            conta(Filtro::Situacao(Estado::Disponivel), &mut c.a_venda);
            conta(Filtro::Situacao(Estado::Comprada), &mut c.compradas);
        }
        c
    }
}

/// O que dá para fazer com um conjunto de fotos escolhidas.
///
/// A tela usa isto para escrever a frase honesta acima dos botões — *"3 podem
/// mudar; comprada e apagada ficam como estão"* — e para saber **quais ids**
/// mandar para a ação. Mandar a seleção inteira faria o servidor recusar metade
/// e a tela mostrar falhas que ela podia ter previsto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Permissoes {
    /// Quantas foram escolhidas.
    pub escolhidas: usize,
    /// Os ids que a ação em lote deve receber — só os que podem mudar.
    pub ids_que_mudam: Vec<String>,
}

impl Permissoes {
    pub fn podem_mudar(&self) -> usize {
        self.ids_que_mudam.len()
    }

    /// Nenhuma das escolhidas aceita mudança — o botão fica desligado.
    pub fn nada_a_fazer(&self) -> bool {
        self.ids_que_mudam.is_empty()
    }

    /// Parte da escolha não vai junto — a tela precisa dizer isso antes.
    pub fn tem_intocaveis(&self) -> bool {
        self.podem_mudar() < self.escolhidas
    }
}

/// As permissões de uma escolha, na ordem da grade.
pub fn permissoes<'a>(escolhidas: impl Iterator<Item = &'a Foto>) -> Permissoes {
    let mut escolhidas_total = 0;
    let mut ids = Vec::new();
    for foto in escolhidas {
        escolhidas_total += 1;
        if foto.editavel() {
            ids.push(foto.id.clone());
        }
    }
    Permissoes {
        escolhidas: escolhidas_total,
        ids_que_mudam: ids,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn foto(id: &str, estado: Estado, apagada: bool) -> Foto {
        Foto {
            rejeitada: false,
            id: id.to_string(),
            arquivo: format!("{id}.jpg"),
            estado,
            apagada,
            produto_efetivo: "avulsa".to_string(),
            preco_negociado: None,
            tem_observacao: false,
            preco_de_venda: None,
            pedido_id: None,
            downloads: 0,
            nota: Some(3),
            revelada: false,
            ordem: 0,
        }
    }

    fn sem_nota(id: &str) -> Foto {
        Foto {
            nota: None,
            ..foto(id, Estado::Disponivel, false)
        }
    }

    #[test]
    fn classificadas_sao_as_que_tem_nota_e_nada_mais() {
        let mut a = Acervo::novo();
        a.definir(vec![
            foto("com-nota", Estado::Disponivel, false),
            sem_nota("sem"),
            foto("apagada", Estado::Disponivel, true),
        ]);
        a.filtrar(Filtro::Classificadas);
        assert_eq!(
            a.visiveis().map(|f| f.id.as_str()).collect::<Vec<_>>(),
            ["com-nota"]
        );
        let c = a.contagens();
        assert_eq!((c.classificadas, c.sem_nota), (1, 1));
    }

    /// 🔑 **"Sinalizada" é a levada no balcão**, e não um recorte próprio: quem
    /// sinaliza é a tecla P, e o que ela escreve é `levada_no_balcao` (dono,
    /// 2026-09-11: *"as levadas são as sinalizadas"*). O chip com esse nome usa
    /// este recorte — a barra não tem dois caminhos para a mesma conta.
    #[test]
    fn sinalizada_e_a_levada_no_balcao_que_a_tecla_p_marca() {
        let mut a = Acervo::novo();
        a.definir(vec![
            foto("marcada", Estado::LevadaNoBalcao, false),
            foto("a-venda", Estado::Disponivel, false),
            sem_nota("sem-nota"),
        ]);
        a.filtrar(Filtro::Situacao(Estado::LevadaNoBalcao));
        assert_eq!(
            a.visiveis().map(|f| f.id.as_str()).collect::<Vec<_>>(),
            ["marcada"]
        );
        let c = a.contagens();
        assert_eq!((c.levadas, c.classificadas), (1, 2));
    }

    fn acervo_de_teste() -> Acervo {
        let mut a = Acervo::novo();
        a.definir(vec![
            foto("a", Estado::Disponivel, false),
            foto("b", Estado::LevadaNoBalcao, false),
            foto("c", Estado::Comprada, false),
            foto("d", Estado::Disponivel, true),
        ]);
        a
    }

    #[test]
    fn o_estado_desconhecido_nao_vira_o_vizinho_mais_proximo() {
        assert_eq!(Estado::do_texto("comprada"), Some(Estado::Comprada));
        assert_eq!(Estado::do_texto("reservada"), None);
        assert_eq!(Estado::do_texto(""), None);
    }

    #[test]
    fn so_muda_o_que_nao_foi_comprado_nem_apagado() {
        assert!(foto("a", Estado::Disponivel, false).editavel());
        assert!(foto("b", Estado::LevadaNoBalcao, false).editavel());
        assert!(!foto("c", Estado::Comprada, false).editavel());
        assert!(!foto("d", Estado::Disponivel, true).editavel());
    }

    #[test]
    fn as_contagens_sao_do_acervo_inteiro_e_nao_do_recorte() {
        let mut a = acervo_de_teste();
        a.filtrar(Filtro::Situacao(Estado::Comprada));
        let c = a.contagens();
        assert_eq!(c.todas, 4);
        assert_eq!(c.a_venda, 1, "a apagada não conta como à venda");
        assert_eq!(c.levadas, 1);
        assert_eq!(c.compradas, 1);
        assert_eq!(c.apagadas, 1);
        assert_eq!(a.total_visivel(), 1, "mas o recorte mostra uma só");
    }

    /// ❌ **Só a rejeitada fica fora da venda** (dono, 2026-09-21).
    ///
    /// A sem nota está à venda e vai à nuvem com marca d'água; a rejeitada sai
    /// de todo recorte menos "Todas" e o dela — "À venda", "Sem nota" e
    /// "Classificadas". O número de cada chip é o que o recorte devolve, em
    /// todos.
    #[test]
    fn a_rejeitada_sai_dos_recortes_e_tem_o_dela() {
        let mut sem = foto("sem", Estado::Disponivel, false);
        sem.nota = None;
        let mut rejeitada = foto("rej", Estado::Disponivel, false);
        rejeitada.rejeitada = true;
        let mut rejeitada_sem_nota = foto("rej-sem", Estado::Disponivel, false);
        rejeitada_sem_nota.rejeitada = true;
        rejeitada_sem_nota.nota = None;
        let mut a = Acervo::novo();
        a.definir(vec![
            foto("com", Estado::Disponivel, false),
            sem,
            rejeitada,
            rejeitada_sem_nota,
        ]);

        let c = a.contagens();
        assert_eq!(
            c.a_venda, 2,
            "a classificada e a sem nota; a rejeitada, não"
        );
        assert_eq!(c.sem_nota, 1, "rejeitada não é sem nota");
        assert_eq!(c.classificadas, 1, "nem classificada");
        assert_eq!(c.rejeitadas, 2);
        assert_eq!(c.todas, 4, "todas continuam existindo");

        a.filtrar(Filtro::Situacao(Estado::Disponivel));
        let ids: Vec<&str> = (0..a.total_visivel())
            .map(|i| a.visivel(i).unwrap().id.as_str())
            .collect();
        assert_eq!(ids, ["com", "sem"]);

        a.filtrar(Filtro::Rejeitadas);
        let ids: Vec<&str> = (0..a.total_visivel())
            .map(|i| a.visivel(i).unwrap().id.as_str())
            .collect();
        assert_eq!(ids, ["rej", "rej-sem"]);

        // O número do chip é o que o recorte devolve, em todo recorte.
        for filtro in [
            Filtro::Todas,
            Filtro::Apagadas,
            Filtro::SemNota,
            Filtro::Classificadas,
            Filtro::Rejeitadas,
            Filtro::Situacao(Estado::Disponivel),
            Filtro::Situacao(Estado::LevadaNoBalcao),
            Filtro::Situacao(Estado::Comprada),
        ] {
            a.filtrar(filtro);
            assert_eq!(a.total_visivel(), c.de(filtro), "{filtro:?}");
        }
    }

    /// 🚨 **O recorte das não classificadas** — o pedido do dono de 2026-09-05.
    ///
    /// Ele existe para achar o que não devia estar aqui: sem nota a foto não
    /// pode ir à venda nem receber marca d'água, e não devia ter subido. A
    /// apagada fica de fora como em todo recorte por situação — ela não tem
    /// arquivo, e classificar o que não existe não leva a lugar nenhum.
    #[test]
    fn o_recorte_sem_nota_junta_o_que_ninguem_classificou() {
        let mut sem = foto("sem", Estado::Disponivel, false);
        sem.nota = None;
        let mut apagada_sem_nota = foto("apagada", Estado::Disponivel, true);
        apagada_sem_nota.nota = None;
        let mut a = Acervo::novo();
        a.definir(vec![
            foto("com", Estado::Disponivel, false),
            sem,
            apagada_sem_nota,
        ]);

        let c = a.contagens();
        assert_eq!(c.sem_nota, 1, "a apagada nao entra na conta");

        a.filtrar(Filtro::SemNota);
        assert_eq!(a.total_visivel(), 1);
        assert_eq!(a.visivel(0).unwrap().id, "sem");
    }

    /// 🚨 A apagada não tem arquivo: oferecê-la como "à venda" seria vender o
    /// que não existe.
    #[test]
    fn a_apagada_so_aparece_no_recorte_dela() {
        let mut a = acervo_de_teste();
        a.filtrar(Filtro::Situacao(Estado::Disponivel));
        assert_eq!(
            a.visiveis().map(|f| f.id.as_str()).collect::<Vec<_>>(),
            vec!["a"]
        );
        a.filtrar(Filtro::Apagadas);
        assert_eq!(
            a.visiveis().map(|f| f.id.as_str()).collect::<Vec<_>>(),
            vec!["d"]
        );
    }

    #[test]
    fn a_posicao_e_a_da_grade_filtrada() {
        let mut a = acervo_de_teste();
        assert_eq!(a.posicao_de("c"), Some(2));
        a.filtrar(Filtro::Situacao(Estado::Comprada));
        assert_eq!(a.posicao_de("c"), Some(0));
        assert_eq!(a.posicao_de("a"), None, "escondida pelo recorte");
    }

    #[test]
    fn a_acao_em_lote_recebe_so_o_que_pode_mudar() {
        let a = acervo_de_teste();
        let p = permissoes(a.todas().iter());
        assert_eq!(p.escolhidas, 4);
        assert_eq!(p.ids_que_mudam, vec!["a".to_string(), "b".to_string()]);
        assert!(p.tem_intocaveis(), "a comprada e a apagada ficam de fora");
        assert!(!p.nada_a_fazer());
    }

    #[test]
    fn escolher_so_compradas_desliga_os_botoes() {
        let a = acervo_de_teste();
        let so_comprada: Vec<&Foto> = a.todas().iter().filter(|f| !f.editavel()).collect();
        let p = permissoes(so_comprada.into_iter());
        assert_eq!(p.escolhidas, 2);
        assert!(p.nada_a_fazer());
    }
}

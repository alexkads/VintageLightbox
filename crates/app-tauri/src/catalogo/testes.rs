use super::*;

fn agora() -> String {
    "2026-09-16T12:00:00Z".to_string()
}

fn com_sessao_e_foto(banco: &Connection) {
    banco
        .execute(
            "INSERT INTO sessoes (id, titulo, atualizada_em) VALUES ('s1', 'Ensaio', ?1)",
            [agora()],
        )
        .unwrap();
    banco
        .execute(
            "INSERT INTO fotos (id, sessao_id, nome_original, ordem, bruto_caminho, bruto_sha256, bruto_bytes, criada_em)
             VALUES ('f1', 's1', 'DSC_0001.jpg', 0, 'fotos/s1/DSC_0001.jpg', ?1, 10, ?2)",
            [&"a".repeat(64), &agora()],
        )
        .unwrap();
}

#[test]
fn um_catalogo_novo_nasce_na_ultima_versao_com_as_pastas() {
    let raiz = tempfile::tempdir().unwrap();
    let catalogo = Catalogo::abrir(raiz.path()).unwrap();
    assert_eq!(catalogo.situacao().unwrap().versao, MIGRACOES.len());
    for pasta in PASTAS {
        assert!(raiz.path().join(pasta).is_dir(), "{pasta}");
    }
    // Um banco novo não precisa de cópia.
    assert!(!raiz.path().join("catalogo.db.antes-v1").exists());
}

#[test]
fn g1_uma_segunda_abertura_e_recusada_enquanto_a_primeira_vive() {
    let raiz = tempfile::tempdir().unwrap();
    let primeiro = Catalogo::abrir(raiz.path()).unwrap();
    assert!(matches!(
        Catalogo::abrir(raiz.path()),
        Err(ErroDoCatalogo::EmUso(_))
    ));
    drop(primeiro);
    assert!(Catalogo::abrir(raiz.path()).is_ok());
}

#[test]
fn g2_reabrir_nao_reaplica_nada() {
    let raiz = tempfile::tempdir().unwrap();
    {
        let catalogo = Catalogo::abrir(raiz.path()).unwrap();
        com_sessao_e_foto(catalogo.banco());
    }
    let catalogo = Catalogo::abrir(raiz.path()).unwrap();
    let fotos: i64 = catalogo
        .banco()
        .query_row("SELECT count(*) FROM fotos", [], |l| l.get(0))
        .unwrap();
    assert_eq!(fotos, 1);
}

#[test]
fn g2_um_catalogo_mais_novo_que_o_app_nao_abre() {
    let raiz = tempfile::tempdir().unwrap();
    {
        let catalogo = Catalogo::abrir(raiz.path()).unwrap();
        catalogo
            .banco()
            .pragma_update(None, "user_version", 99)
            .unwrap();
    }
    match Catalogo::abrir(raiz.path()) {
        Err(ErroDoCatalogo::VersaoMaisNova {
            encontrada: 99,
            conhecida,
        }) => assert_eq!(conhecida, MIGRACOES.len()),
        outro => panic!("esperava VersaoMaisNova, veio {:?}", outro.err()),
    }
}

#[test]
fn g2_uma_migration_que_falha_nao_deixa_nada_pela_metade() {
    let raiz = tempfile::tempdir().unwrap();
    let caminho = raiz.path().join(ARQUIVO_DO_BANCO);
    let mut banco = Connection::open(&caminho).unwrap();
    // Uma tabela com o nome da primeira faz a 0001 falhar no meio.
    banco
        .execute_batch("CREATE TABLE fila (x INTEGER); PRAGMA user_version = 0;")
        .unwrap();
    let erro = migrar(&mut banco, &caminho).unwrap_err();
    assert!(matches!(
        erro,
        ErroDoCatalogo::Migracao {
            nome: "0001_catalogo",
            ..
        }
    ));
    // Nada da 0001 ficou: nem as tabelas anteriores à falha, nem a versão.
    assert_eq!(versao(&banco).unwrap(), 0);
    let sessoes: i64 = banco
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name = 'sessoes'",
            [],
            |l| l.get(0),
        )
        .unwrap();
    assert_eq!(sessoes, 0);
}

#[test]
fn g2_antes_de_atualizar_um_catalogo_com_dados_guarda_uma_copia() {
    let raiz = tempfile::tempdir().unwrap();
    let caminho = raiz.path().join(ARQUIVO_DO_BANCO);
    let mut banco = Connection::open(&caminho).unwrap();
    let primeira = [("0001_teste", "CREATE TABLE a (x INTEGER);")];
    let duas = [
        ("0001_teste", "CREATE TABLE a (x INTEGER);"),
        ("0002_teste", "ALTER TABLE a ADD COLUMN y INTEGER;"),
    ];

    // Banco novo: aplica a 0001 sem cópia, porque não há o que guardar.
    migrar_com(&mut banco, &caminho, &primeira).unwrap();
    assert!(!raiz.path().join("catalogo.db.antes-v1").exists());
    banco.execute("INSERT INTO a (x) VALUES (7)", []).unwrap();

    // Com dados, a 0002 só roda depois da cópia, que guarda o estado de antes.
    migrar_com(&mut banco, &caminho, &duas).unwrap();
    let copia = raiz.path().join("catalogo.db.antes-v2");
    assert!(copia.exists());
    let antes = Connection::open(&copia).unwrap();
    let x: i64 = antes
        .query_row("SELECT x FROM a", [], |l| l.get(0))
        .unwrap();
    assert_eq!(x, 7);
    assert!(antes
        .query_row("SELECT y FROM a", [], |l| l.get::<_, Option<i64>>(0))
        .is_err());
    assert_eq!(versao(&antes).unwrap(), 1);
    assert_eq!(versao(&banco).unwrap(), 2);
}

#[test]
fn g4_o_bruto_nao_muda() {
    let raiz = tempfile::tempdir().unwrap();
    let catalogo = Catalogo::abrir(raiz.path()).unwrap();
    com_sessao_e_foto(catalogo.banco());
    let erro = catalogo
        .banco()
        .execute(
            "UPDATE fotos SET bruto_sha256 = ?1 WHERE id = 'f1'",
            [&"b".repeat(64)],
        )
        .unwrap_err();
    assert!(erro.to_string().contains("o BRUTO não muda"), "{erro}");
    // Os parâmetros mudam: são o estado atual.
    catalogo
        .banco()
        .execute(
            "UPDATE fotos SET parametros = '{\"exposicao\": 0.5}' WHERE id = 'f1'",
            [],
        )
        .unwrap();
}

#[test]
fn g3_a_linha_do_tempo_so_recebe_inclusoes() {
    let raiz = tempfile::tempdir().unwrap();
    let catalogo = Catalogo::abrir(raiz.path()).unwrap();
    com_sessao_e_foto(catalogo.banco());
    catalogo
        .banco()
        .execute(
            "INSERT INTO linha_do_tempo (foto_id, tipo, quando, maquina) VALUES ('f1', 'importada', ?1, 'mac')",
            [agora()],
        )
        .unwrap();
    for sql in [
        "UPDATE linha_do_tempo SET tipo = 'outra'",
        "DELETE FROM linha_do_tempo",
    ] {
        let erro = catalogo.banco().execute(sql, []).unwrap_err();
        assert!(
            erro.to_string().contains("só recebe inclusões"),
            "{sql}: {erro}"
        );
    }
}

#[test]
fn g5_os_parametros_sao_um_objeto_json() {
    let raiz = tempfile::tempdir().unwrap();
    let catalogo = Catalogo::abrir(raiz.path()).unwrap();
    com_sessao_e_foto(catalogo.banco());
    for invalido in ["[1, 2]", "nao é json", "3"] {
        assert!(
            catalogo
                .banco()
                .execute(
                    "UPDATE fotos SET parametros = ?1 WHERE id = 'f1'",
                    [invalido]
                )
                .is_err(),
            "{invalido}"
        );
    }
}

#[test]
fn g6_g7_a_fila_nao_duplica_e_recusa_sem_motivo() {
    let raiz = tempfile::tempdir().unwrap();
    let catalogo = Catalogo::abrir(raiz.path()).unwrap();
    com_sessao_e_foto(catalogo.banco());
    let inserir = "INSERT INTO fila (chave, foto_id, operacao, criada_em, atualizada_em) VALUES ('k1', 'f1', 'enviar', ?1, ?1)";
    catalogo.banco().execute(inserir, [agora()]).unwrap();
    assert!(catalogo.banco().execute(inserir, [agora()]).is_err());
    assert!(catalogo
        .banco()
        .execute("UPDATE fila SET estado = 'recusado' WHERE chave = 'k1'", [])
        .is_err());
    catalogo
        .banco()
        .execute(
            "UPDATE fila SET estado = 'recusado', motivo = '409: já existe' WHERE chave = 'k1'",
            [],
        )
        .unwrap();
}

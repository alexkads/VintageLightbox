-- O nome que o arquivo tinha quando entrou — o que a câmera deu.
--
-- 🚨 **Existe porque o arquivo no disco passou a se chamar UUID.** Proposta do
-- dono (2026-09-08): *"no storage local e cloud a foto poderia ficar com UUID no
-- nome do arquivo e o banco de dados ficaria mais robusto"*. A nuvem já fazia
-- assim — `pos-venda/<uuid>/originais/<uuid>.jpg`, com o nome do cliente numa
-- coluna à parte —, e quem estava fora do padrão era o desktop.
--
-- 🚨 **Sem esta coluna, renomear o arquivo apagaria o nome de dentro do app
-- também.** `Photo::file_name()` era **derivado do caminho**: a grade, a tira, o
-- painel, o título da Revelação, a tela do cliente, o nome do arquivo exportado
-- e o nome com que a foto sobe para a galeria do cliente (`publicar::subir`)
-- saíam todos dali. O operador perderia `DSC_2571.JPG` em todos eles, e o
-- cliente veria um UUID na galeria dele.
--
-- 🔑 **O que o UUID compra é o caminho parar de carregar significado.** Duas
-- fotos `DSC_2571.jpg`, de dois cartões, no mesmo ensaio: o organizador resolvia
-- renomeando a segunda para `DSC_2571_1.jpg` — e o app passava a chamá-la por um
-- nome que não existe em lugar nenhum além do nosso disco.
ALTER TABLE photos ADD COLUMN nome_original TEXT;

-- Preenche as que já existem com o nome que está no caminho — o mesmo que
-- `file_name()` devolvia antes desta coluna.
--
-- 🔑 **O idioma do SQLite para "basename"**: `replace(caminho,'/','')` dá o
-- caminho sem barras; `rtrim` tira do fim tudo o que estiver nesse conjunto,
-- sobrando o prefixo até a última barra; e o `replace` externo remove esse
-- prefixo. É a forma canônica, e não precisa de extensão nenhuma.
--
-- ⚠️ **Isto é conveniência, e não correção.** Um caminho do Windows (`\`) ou uma
-- linha em que o prefixo se repita dentro do nome saem errados aqui — e é por
-- isso que `file_name()` **continua caindo no caminho** quando a coluna é NULL
-- ou está vazia. Nenhuma foto antiga depende deste UPDATE para ter nome.
UPDATE photos
   SET nome_original = replace(
           file_path,
           rtrim(file_path, replace(file_path, '/', '')),
           ''
       )
 WHERE nome_original IS NULL
   AND file_path LIKE '%/%';

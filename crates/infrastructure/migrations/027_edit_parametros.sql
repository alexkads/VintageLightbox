-- Fase B do contrato "revelar e editar" (recordarfotos-e-commerce/docs/REVELAR_E_EDITAR.md):
-- a revelação inteira da foto (migration 023) passa a se chamar pelos parâmetros.
ALTER TABLE photos RENAME COLUMN edit_receita TO edit_parametros;

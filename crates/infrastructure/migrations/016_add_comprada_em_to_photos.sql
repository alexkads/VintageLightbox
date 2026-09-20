-- Quando o cliente levou a foto no balcão (RFC 3339, como as outras datas).
-- NULL = ficou para trás — é a que vai à venda com marca d'água no pós-venda.
ALTER TABLE photos ADD COLUMN comprada_em TEXT;

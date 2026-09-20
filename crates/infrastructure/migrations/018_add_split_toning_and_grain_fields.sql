-- Tonalização (split toning) e grão de filme.
--
-- Os sete campos que o motor ganhou em 2026-09-06, quando o `Ajustes` passou de
-- 46 para 53. Sem colunas, os sliders novos moveriam a foto na tela e sumiriam
-- ao reabrir — o mesmo estado que o site tem para o preset importado.
--
-- O padrão é 0.0 em todos, e é o neutro do motor: matiz sem saturação não pinta
-- nada, e grão em zero não entra no bloco do shader.
ALTER TABLE photos ADD COLUMN edit_split_shadow_hue REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_split_shadow_sat REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_split_highlight_hue REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_split_highlight_sat REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_split_balance REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_grain_amount REAL DEFAULT 0.0;
ALTER TABLE photos ADD COLUMN edit_grain_size REAL DEFAULT 0.0;

-- Uma predefinição passa a guardar qualquer um dos 53 ajustes, e não 15.
--
-- 🚨 **A tabela tinha uma coluna por campo, e a lista parou em 2026-02.** São os
-- 11 do Básico e os 4 da curva de tons — os que o motor tinha quando ela foi
-- escrita. Depois entraram HSL, ruído, nitidez, lente, tonalização e grão, e
-- nenhuma predefinição podia guardá-los: salvar "Sépia" gravava a saturação e
-- **descartava calado** a tonalização que faz a sépia ser sépia.
--
-- Agora é um mapa `nome → valor` em JSON, com os nomes de `Ajustes::NOMES` — os
-- mesmos do shader e os mesmos que o site usa. Campo ausente não é neutro: é
-- "esta predefinição não mexe nisso", que é o que permite somar uma de cor com
-- uma de nitidez.
--
-- ⚠️ **As de sistema não estão aqui** — nascem em código (`presets_de_sistema`),
-- versionadas junto com o motor que as interpreta. O que esta conversão carrega
-- é só o que o fotógrafo salvou.
CREATE TABLE presets_com_mapa (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    is_system BOOLEAN NOT NULL DEFAULT 0,
    adjustments TEXT NOT NULL DEFAULT '{}',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 🔑 `WHERE valor.value IS NOT NULL` é o que preserva o significado antigo: uma
-- coluna nula queria dizer "não mexe neste campo", e um `json_object` cru a
-- levaria como `null` — que na leitura viraria campo presente, com valor
-- inventado.
INSERT INTO presets_com_mapa (id, name, is_system, adjustments, created_at, updated_at)
SELECT
    velho.id,
    velho.name,
    velho.is_system,
    COALESCE(
        (
            SELECT json_group_object(campo.key, campo.value)
            FROM json_each(
                json_object(
                    'exposure', velho.exposure,
                    'contrast', velho.contrast,
                    'temperature', velho.temperature,
                    'tint', velho.tint,
                    'highlights', velho.highlights,
                    'shadows', velho.shadows,
                    'whites', velho.whites,
                    'blacks', velho.blacks,
                    'clarity', velho.clarity,
                    'vibrance', velho.vibrance,
                    'saturation', velho.saturation,
                    'tone_curve_shadows', velho.tone_curve_shadows,
                    'tone_curve_darks', velho.tone_curve_darks,
                    'tone_curve_lights', velho.tone_curve_lights,
                    'tone_curve_highlights', velho.tone_curve_highlights
                )
            ) AS campo
            WHERE campo.value IS NOT NULL
        ),
        '{}'
    ),
    velho.created_at,
    velho.updated_at
FROM presets AS velho;

DROP TABLE presets;
ALTER TABLE presets_com_mapa RENAME TO presets;
CREATE INDEX IF NOT EXISTS idx_presets_name ON presets(name);

--[[
A régua: uma foto revelada de cada jeito da lista, um JPEG por caso, com a
revelação no XMP do próprio arquivo — de onde o `comparar_com_o_lightroom` a
lê. Usada pelo menu (Calibrar.lua) e pelo pedido automático (Iniciar.lua).

🚨 **Cada caso começa do "Redefinir" do Revelar**, que age na foto ativa.
Antes de cada um, a foto ativa é conferida: se não for a da régua (alguém
clicou em outra), a régua para — redefinir a foto de um operador apagaria o
trabalho dele.
]]

local LrApplication = import "LrApplication"
local LrApplicationView = import "LrApplicationView"
local LrDevelopController = import "LrDevelopController"
local LrExportSession = import "LrExportSession"
local LrFileUtils = import "LrFileUtils"
local LrPathUtils = import "LrPathUtils"
local LrTasks = import "LrTasks"

local Regua = {}

-- Os perfis criativos não se aplicam por nome pelo SDK; vêm da predefinição
-- do estúdio que os usa, com todo o resto dela devolvido ao neutro.
local PERFIS = {
	{ "perfil-modern09", "RecordarFotos Colorido Quente" },
	{ "perfil-vintage10", "RecordarFotos Colorido Envelhecido" },
	{ "perfil-bw01", "RecordarFotos P&B Perfurado" },
	{ "perfil-bw10", "RecordarFotos P&B" },
}

-- Um slider por vez, nos valores que as predefinições do estúdio usam e nas
-- pontas da escala.
local SLIDERS = {
	{ "Exposure2012", { -1, 1 } },
	{ "Contrast2012", { -100, -57, 50, 100 } },
	{ "Highlights2012", { -100, -38, 100 } },
	{ "Shadows2012", { -100, 45, 78, 100 } },
	{ "Whites2012", { -50, 50 } },
	{ "Blacks2012", { -100, -56, 50 } },
	{ "Clarity2012", { -50, 50 } },
	{ "Dehaze", { 30 } },
	{ "Vibrance", { 50 } },
	{ "Saturation", { -45, 50 } },
}

local function aparar(texto)
	return (texto:gsub("^%s+", ""):gsub("%s+$", ""))
end

local function arquivo(nome)
	return (nome:gsub("[^%w%+%-%.]", "_"))
end

local registro_caminho
function Regua.registrar(texto)
	if not registro_caminho then
		return
	end
	local f = io.open(registro_caminho, "a")
	if f then
		f:write(os.date("%H:%M:%S "), texto, "\n")
		f:close()
	end
end

-- As predefinições pelo nome (aparado: há "RecordarFotos P&B " com espaço).
local function predefinicoes()
	local por_nome = {}
	for _, pasta in ipairs(LrApplication.developPresetFolders()) do
		for _, p in ipairs(pasta:getDevelopPresets()) do
			por_nome[aparar(p:getName())] = p
		end
	end
	return por_nome
end

-- As do estúdio, inteiras: é a régua de "faz o mesmo que o Lightroom".
local function do_estudio(por_nome)
	local nomes = {}
	for nome in pairs(por_nome) do
		if nome:find("^RecordarFotos") or nome:find("^Recordarfotos") or nome:find("^Vinheta")
			or nome == "Colorido envelhacido" or nome == "Predefinição sem título" then
			table.insert(nomes, nome)
		end
	end
	table.sort(nomes)
	return nomes
end

-- A vinheta pós-corte, uma variável por vez a partir da "Vinheta Carregada"
-- (−61, ponto médio 50, difusão 50, arredondamento 0, prioridade de realces).
-- Numa foto cinza lisa, o pixel exportado é o próprio desenho da vinheta.
local function casos_de_vinheta()
	local base = {
		PostCropVignetteAmount = -61,
		PostCropVignetteMidpoint = 50,
		PostCropVignetteFeather = 50,
		PostCropVignetteRoundness = 0,
		PostCropVignetteStyle = 1,
		PostCropVignetteHighlightContrast = 0,
	}
	local variacoes = {
		{ "Amount", { -100, -61, -40, -20, 20, 40, 100 } },
		{ "Midpoint", { 0, 25, 75, 100 } },
		{ "Feather", { 0, 25, 75, 100 } },
		{ "Roundness", { -100, -83, -50, 50, 100 } },
		{ "Style", { 2, 3 } },
	}
	local casos = { { nome = "00-neutro", ajustes = {} } }
	for _, v in ipairs(variacoes) do
		for _, valor in ipairs(v[2]) do
			local ajustes = {}
			for k, x in pairs(base) do
				ajustes[k] = x
			end
			ajustes["PostCropVignette" .. v[1]] = valor
			table.insert(casos, {
				nome = string.format("vinheta-%s%s%s", v[1], valor > 0 and "+" or "", tostring(valor)),
				ajustes = ajustes,
			})
		end
	end
	-- O balanço de branco. 🚨 Pelo SDK, `IncrementalTint` sozinho é ignorado
	-- (a régua de 1/out saiu com "As Shot" e zero): vai `WhiteBalance =
	-- "Custom"` com `Temperature`/`Tint`, e os incrementais junto.
	for _, b in ipairs { { "Tint", { -50, -23, 23 } }, { "Temperature", { -30, 8, 30 } } } do
		for _, valor in ipairs(b[2]) do
			table.insert(casos, {
				nome = string.format("balanco-%s%s%s", b[1], valor > 0 and "+" or "", tostring(valor)),
				ajustes = {
					WhiteBalance = "Custom",
					[b[1]] = valor,
					["Incremental" .. b[1]] = valor,
				},
			})
		end
	end
	-- A "Vinheta Borda" inteira: sobreposição branca, a mais usada no P&B.
	table.insert(casos, {
		nome = "vinheta-borda",
		ajustes = {
			PostCropVignetteAmount = 100,
			PostCropVignetteMidpoint = 0,
			PostCropVignetteFeather = 35,
			PostCropVignetteRoundness = -83,
			PostCropVignetteStyle = 3,
			PostCropVignetteHighlightContrast = 0,
		},
	})
	return casos
end

local function vinheta(quantidade, meio, difusao, arred, estilo)
	return {
		PostCropVignetteAmount = quantidade,
		PostCropVignetteMidpoint = meio,
		PostCropVignetteFeather = difusao,
		PostCropVignetteRoundness = arred,
		PostCropVignetteStyle = estilo,
		PostCropVignetteHighlightContrast = 0,
	}
end

-- As nove combinações que as predefinições do estúdio usam.
local DO_ESTUDIO = {
	{ -61, 50, 50, 0, 1 }, { -40, 50, 50, 0, 1 }, { -37, 50, 50, 0, 1 }, { -33, 50, 50, 0, 1 },
	{ 100, 39, 24, 11, 1 }, { 32, 32, 68, 0, 1 }, { 47, 0, 46, -80, 2 },
	{ -66, 0, 48, -61, 3 }, { 78, 0, 48, -61, 3 }, { 100, 0, 35, -83, 3 },
}

-- A grade da vinheta: a forma (ponto médio × difusão), o arredondamento e as
-- combinações do estúdio — numa foto cinza lisa.
local function casos_de_grade()
	local casos = { { nome = "00-neutro", ajustes = {} } }
	local passos = { 0, 13, 25, 38, 50, 63, 75, 88, 100 }
	for _, meio in ipairs(passos) do
		for _, dif in ipairs(passos) do
			table.insert(casos, {
				nome = string.format("grade-m%03d-f%03d", meio, dif),
				ajustes = vinheta(-61, meio, dif, 0, 1),
			})
		end
	end
	for _, arred in ipairs { -100, -83, -80, -61, -50, -25, 11, 25, 50, 75, 100 } do
		table.insert(casos, {
			nome = string.format("arred%+04d", arred),
			ajustes = vinheta(-61, 50, 50, arred, 1),
		})
	end
	for i, c in ipairs(DO_ESTUDIO) do
		table.insert(casos, {
			nome = string.format("estudio%02d-a%+04d-m%03d-f%03d-r%+04d-s%d", i, c[1], c[2], c[3], c[4], c[5]),
			ajustes = vinheta(c[1], c[2], c[3], c[4], c[5]),
		})
	end
	return casos
end

-- A força: cada estilo, de −100 a +100, no meio/difusão/arredondamento do
-- estúdio (50/50/0) — em cada nível de cinza, para ver como o efeito
-- depende do tom.
local function casos_de_forca()
	local casos = { { nome = "00-neutro", ajustes = {} } }
	for estilo = 1, 3 do
		for _, q in ipairs { -100, -80, -61, -40, -20, 20, 40, 60, 80, 100 } do
			table.insert(casos, {
				nome = string.format("forca-s%d-a%+04d", estilo, q),
				ajustes = vinheta(q, 50, 50, 0, estilo),
			})
		end
	end
	return casos
end

-- O tom: cada slider do Básico numa varredura densa, para a curva de cada
-- valor sair da rampa (cinza e colorida).
local function casos_de_tom()
	local casos = { { nome = "00-neutro", ajustes = {} } }
	local function varrer(chave, valores, formato)
		for _, v in ipairs(valores) do
			table.insert(casos, { nome = string.format(formato, chave, v), ajustes = { [chave] = v } })
		end
	end
	local cem = {}
	for v = -100, 100, 10 do
		if v ~= 0 then
			table.insert(cem, v)
		end
	end
	varrer("Exposure2012", { -2, -1.5, -1, -0.75, -0.5, -0.25, 0.25, 0.5, 0.75, 1, 1.5, 2 }, "tom-%s%+.2f")
	for _, chave in ipairs { "Contrast2012", "Highlights2012", "Shadows2012", "Whites2012", "Blacks2012" } do
		varrer(chave, cem, "tom-%s%+04d")
	end
	return casos
end

-- A força de 10 em 10, para a tabela F(estilo, valor, quantidade): em fotos
-- de quatro quadrantes, cada canto mede um nível por exportação.
local function casos_de_forca_fina()
	local casos = { { nome = "00-neutro", ajustes = {} } }
	for estilo = 1, 3 do
		for q = -100, 100, 10 do
			if q ~= 0 then
				table.insert(casos, {
					nome = string.format("forca-s%d-a%+04d", estilo, q),
					ajustes = vinheta(q, 50, 50, 0, estilo),
				})
			end
		end
	end
	return casos
end

-- Só a Exposição, de −5 a +5 — o alcance inteiro do slider (a régua de tom
-- parava em ±2, e o processo 1 travava a foto dali em diante).
local function casos_de_exposicao()
	local casos = { { nome = "00-neutro", ajustes = {} } }
	for _, ev in ipairs { -5, -4, -3, -2.5, -2, -1.5, -1, -0.5, -0.25, 0.25, 0.5, 1, 1.5, 2, 2.5, 3, 4, 5 } do
		table.insert(casos, { nome = string.format("exposicao%+.2f", ev), ajustes = { Exposure2012 = ev } })
	end
	return casos
end

-- Cada controle do Básico (menos a Exposição, que tem régua própria) sozinho,
-- de −100 a +100, numa foto de verdade. O balanço vai como "Personalizado",
-- com a chave do SDK e a incremental (só a incremental é ignorada).
local function casos_de_controles()
	local casos = { { nome = "00-neutro", ajustes = {} } }
	local valores = { -100, -50, -25, 25, 50, 100 }
	for _, chave in ipairs {
		"Contrast2012", "Highlights2012", "Shadows2012", "Whites2012", "Blacks2012",
		"Texture", "Clarity2012", "Dehaze", "Vibrance", "Saturation",
	} do
		for _, v in ipairs(valores) do
			table.insert(casos, { nome = string.format("%s%+04d", chave, v), ajustes = { [chave] = v } })
		end
	end
	for _, chave in ipairs { "Temperature", "Tint" } do
		for _, v in ipairs(valores) do
			table.insert(casos, {
				nome = string.format("%s%+04d", chave, v),
				ajustes = { WhiteBalance = "Custom", [chave] = v, ["Incremental" .. chave] = v },
			})
		end
	end
	return casos
end

-- O balanço de branco de −100 a +100, de 10 em 10, Temperatura e Matiz cada
-- um sozinho — nas fotos de quadrantes, 12 níveis de cinza por caso.
local function casos_de_balanco()
	local casos = { { nome = "00-neutro", ajustes = {} } }
	for _, chave in ipairs { "Temperature", "Tint" } do
		for v = -100, 100, 10 do
			if v ~= 0 then
				table.insert(casos, {
					nome = string.format("%s%+04d", chave, v),
					ajustes = { WhiteBalance = "Custom", [chave] = v, ["Incremental" .. chave] = v },
				})
			end
		end
	end
	return casos
end

-- Os componentes de cada predefinição do estúdio: as chaves dela de cada
-- painel, sozinhas. É o que separa "o preset diverge" de "a curva do preset
-- diverge".
local COMPONENTES = {
	{ "basico", { "Exposure2012", "Contrast2012", "Highlights2012", "Shadows2012", "Whites2012", "Blacks2012",
		"Clarity2012", "Texture", "Dehaze", "Vibrance", "Saturation" } },
	{ "balanco", { "WhiteBalance", "Temperature", "Tint", "IncrementalTemperature", "IncrementalTint" } },
	{ "curva", { "ToneCurve", "Parametric" } },
	{ "pb", { "ConvertToGrayscale", "GrayMixer" } },
	{ "hsl", { "HueAdjustment", "SaturationAdjustment", "LuminanceAdjustment" } },
	{ "tonalizacao", { "SplitToning", "ColorGrade" } },
	{ "vinheta", { "PostCropVignette" } },
	{ "detalhe", { "Sharpen", "Sharpness", "LuminanceSmoothing", "LuminanceNoise", "ColorNoiseReduction", "Grain" } },
}

local function comeca_com_algum(chave, prefixos)
	for _, p in ipairs(prefixos) do
		if chave:sub(1, #p) == p then
			return true
		end
	end
	return false
end

local function casos_de_componentes(por_nome)
	local casos = { { nome = "00-neutro", ajustes = {} } }
	for _, nome in ipairs(do_estudio(por_nome)) do
		local ok, todos = LrTasks.pcall(function()
			return por_nome[nome]:getSetting()
		end)
		if ok and todos then
			for _, comp in ipairs(COMPONENTES) do
				local parte = {}
				for chave, valor in pairs(todos) do
					if comeca_com_algum(chave, comp[2]) then
						parte[chave] = valor
					end
				end
				if next(parte) then
					table.insert(casos, {
						nome = string.format("comp-%s-%s", comp[1], nome),
						ajustes = parte,
					})
				end
			end
		end
	end
	return casos
end

-- Vibração e Saturação de −100 a +100, de 10 em 10 — na carta de cores
-- (24 matizes × 6 saturações × 3 brilhos), para tabelar quanto croma cada cor
-- perde ou ganha.
local function casos_de_cor()
	local casos = { { nome = "00-neutro", ajustes = {} } }
	for _, chave in ipairs { "Vibrance", "Saturation" } do
		for v = -100, 100, 10 do
			if v ~= 0 then
				table.insert(casos, { nome = string.format("%s%+04d", chave, v), ajustes = { [chave] = v } })
			end
		end
	end
	return casos
end

-- Remover névoa de −100 a +100, de 25 em 25: o Lightroom decide a névoa por
-- foto (o mesmo −100 leva o preto a 110 numa e a 49 noutra), então a régua é
-- de muitas fotos e poucos valores.
local function casos_de_nevoa()
	local casos = { { nome = "00-neutro", ajustes = {} } }
	for _, v in ipairs { -100, -75, -50, -25, 25, 50, 75, 100 } do
		table.insert(casos, { nome = string.format("Dehaze%+04d", v), ajustes = { Dehaze = v } })
	end
	return casos
end

-- A ordem da vinheta e da viragem: numa foto em P&B, a vinheta branca
-- (pós-corte e de lente) sozinha e com uma viragem sépia. Se a borda clareada
-- sai sépia, o Lightroom vira depois da vinheta.
local function casos_de_vinheta_viragem()
	local pb = { ConvertToGrayscale = true }
	local sepia = {
		SplitToningShadowHue = 45, SplitToningShadowSaturation = 40,
		SplitToningHighlightHue = 45, SplitToningHighlightSaturation = 40,
	}
	local pos = {
		PostCropVignetteAmount = 60, PostCropVignetteStyle = 1,
		PostCropVignetteMidpoint = 30, PostCropVignetteFeather = 50,
	}
	local lente = { VignetteAmount = 60, VignetteMidpoint = 30 }
	local function junta(...)
		local t = {}
		for _, parte in ipairs { ... } do
			for k, v in pairs(parte) do
				t[k] = v
			end
		end
		return t
	end
	return {
		{ nome = "00-neutro", ajustes = {} },
		{ nome = "01-pb", ajustes = junta(pb) },
		{ nome = "02-pb-sepia", ajustes = junta(pb, sepia) },
		{ nome = "03-pb-pos", ajustes = junta(pb, pos) },
		{ nome = "04-pb-sepia-pos", ajustes = junta(pb, sepia, pos) },
		{ nome = "05-pb-lente", ajustes = junta(pb, lente) },
		{ nome = "06-pb-sepia-lente", ajustes = junta(pb, sepia, lente) },
	}
end

function Regua.casos(completa, por_nome, tipo)
	if tipo == "vinheta-viragem" then
		return casos_de_vinheta_viragem()
	end
	if tipo == "cor" then
		return casos_de_cor()
	end
	if tipo == "nevoa" then
		return casos_de_nevoa()
	end
	if tipo == "componentes" then
		return casos_de_componentes(por_nome)
	end
	if tipo == "balanco" then
		return casos_de_balanco()
	end
	if tipo == "exposicao" then
		return casos_de_exposicao()
	elseif tipo == "controles" then
		return casos_de_controles()
	end
	if tipo == "tom" then
		return casos_de_tom()
	elseif tipo == "vinheta-forca-fina" then
		return casos_de_forca_fina()
	end
	if tipo == "vinheta" then
		return casos_de_vinheta()
	elseif tipo == "vinheta-grade" then
		return casos_de_grade()
	elseif tipo == "vinheta-forca" then
		return casos_de_forca()
	end
	local casos = { { nome = "00-neutro", ajustes = {} } }
	if tipo == "sliders" then
		-- Só os sliders: numa rampa de cinza, é a curva de cada um.
		for _, s in ipairs(SLIDERS) do
			for _, v in ipairs(s[2]) do
				table.insert(casos, {
					nome = string.format("slider-%s%s%s", s[1], v > 0 and "+" or "", tostring(v)),
					ajustes = { [s[1]] = v },
				})
			end
		end
		return casos
	end
	for _, nome in ipairs(do_estudio(por_nome)) do
		table.insert(casos, { nome = "preset-" .. nome, preset = nome })
	end
	if completa then
		for _, p in ipairs(PERFIS) do
			table.insert(casos, { nome = p[1], preset = p[2], so_perfil = true })
		end
		for _, s in ipairs(SLIDERS) do
			for _, v in ipairs(s[2]) do
				local sinal = v > 0 and "+" or ""
				table.insert(casos, {
					nome = string.format("slider-%s%s%s", s[1], sinal, tostring(v)),
					ajustes = { [s[1]] = v },
				})
			end
		end
	end
	return casos
end

-- O que o perfil traz e o neutro não pode apagar.
local DO_PERFIL = { Look = true, ConvertToGrayscale = true, CameraProfile = true, CameraProfileDigest = true }

local function eh_a_ativa(catalogo, foto)
	local ativa = catalogo:getTargetPhoto()
	return ativa ~= nil and ativa.localIdentifier == foto.localIdentifier
end

-- Deixa a foto ativa no Revelar; devolve false se não conseguiu.
function Regua.ativar(catalogo, foto)
	LrApplicationView.switchToModule("develop")
	catalogo:setSelectedPhotos(foto, {})
	for _ = 1, 20 do
		LrTasks.sleep(0.25)
		if eh_a_ativa(catalogo, foto) then
			return true
		end
	end
	return false
end

local function revelar(catalogo, foto, caso, neutro, por_nome)
	if not eh_a_ativa(catalogo, foto) then
		return "a foto ativa mudou — régua interrompida", true
	end
	LrDevelopController.resetAllDevelopAdjustments()
	LrTasks.sleep(0.3)
	local erro
	catalogo:withWriteAccessDo("Régua do VintageLightbox: " .. caso.nome, function()
		if caso.preset then
			local p = por_nome[caso.preset]
			if not p then
				erro = "predefinição não encontrada: " .. caso.preset
				return
			end
			foto:applyDevelopPreset(p, _PLUGIN)
		end
		if caso.so_perfil then
			local resto = {}
			for chave, valor in pairs(neutro) do
				if not DO_PERFIL[chave] then
					resto[chave] = valor
				end
			end
			foto:applyDevelopSettings(resto)
		end
		if caso.ajustes and next(caso.ajustes) then
			foto:applyDevelopSettings(caso.ajustes)
		end
	end, { timeout = 30 })
	LrTasks.sleep(0.3)
	return erro
end

local function exportar(foto, pasta, nome)
	local sessao = LrExportSession {
		photosToExport = { foto },
		exportSettings = {
			LR_export_destinationType = "specificFolder",
			LR_export_destinationPathPrefix = pasta,
			LR_export_useSubfolder = false,
			LR_format = "JPEG",
			LR_jpeg_quality = 0.92,
			LR_export_colorSpace = "sRGB",
			-- 2048 no lado maior: o Lightroom revela a foto inteira e reduz
			-- depois, então a revelação é a mesma — e o arquivo cai de ~13 MB
			-- para ~1 MB (a régua inteira encheu o disco em tamanho cheio).
			LR_size_doConstrain = true,
			LR_size_resizeType = "longEdge",
			LR_size_maxWidth = 2048,
			LR_size_maxHeight = 2048,
			LR_size_units = "pixels",
			LR_size_doNotEnlarge = true,
			LR_outputSharpeningOn = false,
			LR_minimizeEmbeddedMetadata = false,
			LR_removeLocationMetadata = false,
			LR_renamingTokensOn = true,
			LR_tokens = "{{custom_token}}",
			LR_tokenCustomString = nome,
			LR_collisionHandling = "overwrite",
		},
	}
	for _, r in sessao:renditions() do
		local ok, caminho = r:waitForRender()
		if not ok then
			return caminho
		end
	end
end

-- Roda os casos numa foto já ativa. Devolve as linhas do relatório e se a
-- régua foi interrompida.
function Regua.rodar(catalogo, foto, pasta, completa, progresso, tipo)
	LrFileUtils.createAllDirectories(pasta)
	LrDevelopController.resetAllDevelopAdjustments()
	LrTasks.sleep(0.5)
	local neutro = foto:getDevelopSettings()
	local por_nome = predefinicoes()
	local casos = Regua.casos(completa, por_nome, tipo)
	local relatorio = {}
	for i, caso in ipairs(casos) do
		if progresso and progresso:isCanceled() then
			break
		end
		if progresso then
			progresso:setPortionComplete(i - 1, #casos)
			progresso:setCaption(caso.nome)
		end
		local erro, parar = revelar(catalogo, foto, caso, neutro, por_nome)
		if not erro then
			erro = exportar(foto, pasta, string.format("%02d-%s", i, arquivo(caso.nome)))
		end
		local linha = string.format("%02d\t%s\t%s", i, caso.nome, erro or "ok")
		table.insert(relatorio, linha)
		Regua.registrar(LrPathUtils.leafName(pasta) .. " " .. linha)
		if parar then
			return relatorio, true
		end
	end
	local saida = io.open(LrPathUtils.child(pasta, "casos.txt"), "w")
	if saida then
		saida:write(table.concat(relatorio, "\n"), "\n")
		saida:close()
	end
	return relatorio, false
end

function Regua.definir_registro(caminho)
	registro_caminho = caminho
end

return Regua

--[[
O pedido automático: com o plug-in carregado, ele olha a cada poucos segundos
por um `pedido.txt` na pasta do próprio plug-in e, se achar, roda a régua sem
ninguém clicar em nada. É o que permite medir as 26 predefinições do estúdio
em várias fotos a partir de um roteiro.

O pedido, uma chave por linha:

    saida=C:\...\regua
    foto=C:\...\original-1.JPG      (a primeira leva também perfis e sliders)
    foto=C:\...\original-2.NEF
    casos=vinheta                   (opcional: só a varredura da vinheta)

As fotos entram no catálogo (se ainda não estão) e na coleção
"Régua VintageLightbox". Use cópias dos originais, numa pasta só da régua: a
revelação delas é redefinida a cada caso.

Ao começar, o pedido vira `pedido-em-andamento.txt`; ao terminar,
`pedido-feito.txt`. O andamento vai para `<saida>\registro.txt`.
]]

local LrApplication = import "LrApplication"
local LrFileUtils = import "LrFileUtils"
local LrPathUtils = import "LrPathUtils"
local LrTasks = import "LrTasks"

local Regua = require "Regua"

local function ler_pedido(caminho)
	local pedido = { fotos = {} }
	for linha in io.lines(caminho) do
		local chave, valor = linha:match("^%s*(%w+)%s*=%s*(.-)%s*$")
		if chave == "saida" then
			pedido.saida = valor
		elseif chave == "foto" then
			table.insert(pedido.fotos, valor)
		elseif chave == "casos" then
			pedido.casos = valor
		end
	end
	return pedido
end

local function atender(caminho_do_pedido)
	local em_andamento = LrPathUtils.child(_PLUGIN.path, "pedido-em-andamento.txt")
	LrFileUtils.delete(em_andamento)
	LrFileUtils.move(caminho_do_pedido, em_andamento)
	local pedido = ler_pedido(em_andamento)
	if not pedido.saida or #pedido.fotos == 0 then
		return
	end
	LrFileUtils.createAllDirectories(pedido.saida)
	Regua.definir_registro(LrPathUtils.child(pedido.saida, "registro.txt"))
	Regua.registrar("pedido com " .. #pedido.fotos .. " foto(s)")

	local catalogo = LrApplication.activeCatalog()
	local fotos = {}
	-- 🚨 A coleção não se usa na mesma transação em que nasce ("Can't get
	-- collection information after creating collection inside the same
	-- withWriteAccessDo"): criar numa, preencher na outra.
	local colecao
	catalogo:withWriteAccessDo("Régua do VintageLightbox: coleção", function()
		colecao = catalogo:createCollection("Régua VintageLightbox", nil, true)
	end, { timeout = 30 })
	catalogo:withWriteAccessDo("Régua do VintageLightbox: importar", function()
		for _, caminho in ipairs(pedido.fotos) do
			local foto = catalogo:findPhotoByPath(caminho)
			if not foto then
				foto = catalogo:addPhoto(caminho)
			end
			if foto then
				table.insert(fotos, { foto = foto, caminho = caminho })
				colecao:addPhotos { foto }
			else
				Regua.registrar("não importou: " .. caminho)
			end
		end
	end, { timeout = 120 })
	catalogo:setActiveSources { colecao }
	Regua.registrar(#fotos .. " foto(s) na coleção")
	LrTasks.sleep(2)

	for i, item in ipairs(fotos) do
		local pasta = LrPathUtils.child(pedido.saida, LrPathUtils.removeExtension(LrPathUtils.leafName(item.caminho)))
		if not Regua.ativar(catalogo, item.foto) then
			Regua.registrar("não consegui deixar ativa: " .. item.caminho .. " — régua interrompida")
			break
		end
		Regua.registrar("foto " .. i .. ": " .. item.caminho)
		local _, interrompida = Regua.rodar(catalogo, item.foto, pasta, i == 1, nil, pedido.casos)
		if interrompida then
			Regua.registrar("régua interrompida")
			break
		end
	end
	Regua.registrar("fim")
	local feito = LrPathUtils.child(_PLUGIN.path, "pedido-feito.txt")
	LrFileUtils.delete(feito)
	LrFileUtils.move(em_andamento, feito)
end

LrTasks.startAsyncTask(function()
	local caminho = LrPathUtils.child(_PLUGIN.path, "pedido.txt")
	while true do
		if LrFileUtils.exists(caminho) then
			local ok, erro = LrTasks.pcall(atender, caminho)
			if not ok then
				Regua.registrar("erro: " .. tostring(erro))
			end
		end
		LrTasks.sleep(5)
	end
end)

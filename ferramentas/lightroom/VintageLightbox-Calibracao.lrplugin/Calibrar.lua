--[[
A régua pelo menu: na cópia virtual selecionada, para uma pasta escolhida.
O trabalho está em Regua.lua; o pedido automático, em Iniciar.lua.

🚨 **Só numa cópia virtual.** Cada caso começa do "Redefinir" do Revelar, e a
revelação da foto se perde no caminho.
]]

local LrApplication = import "LrApplication"
local LrDialogs = import "LrDialogs"
local LrFunctionContext = import "LrFunctionContext"
local LrPathUtils = import "LrPathUtils"
local LrProgressScope = import "LrProgressScope"
local LrTasks = import "LrTasks"

local Regua = require "Regua"

LrTasks.startAsyncTask(function()
	LrFunctionContext.callWithContext("regua", function(contexto)
		local catalogo = LrApplication.activeCatalog()
		local alvos = catalogo:getTargetPhotos()
		if #alvos ~= 1 or not alvos[1]:getRawMetadata("isVirtualCopy") then
			LrDialogs.message(
				"Selecione uma cópia virtual",
				"A régua redefine a revelação a cada caso. Crie uma cópia virtual da foto "
					.. "(Ctrl+') e selecione só ela.",
				"info"
			)
			return
		end
		local foto = alvos[1]

		local escolha = LrDialogs.runOpenPanel {
			title = "Pasta para a régua (uma pasta vazia)",
			canChooseFiles = false,
			canChooseDirectories = true,
			canCreateDirectories = true,
			allowsMultipleSelection = false,
		}
		if not escolha then
			return
		end
		local pasta = escolha[1]
		Regua.definir_registro(LrPathUtils.child(pasta, "registro.txt"))

		if not Regua.ativar(catalogo, foto) then
			LrDialogs.message("A foto não ficou ativa no Revelar", "Nada foi alterado.", "info")
			return
		end
		local progresso = LrProgressScope { title = "Régua do VintageLightbox", functionContext = contexto }
		local relatorio, interrompida = Regua.rodar(catalogo, foto, pasta, true, progresso)
		progresso:done()

		local falhas = 0
		for _, linha in ipairs(relatorio) do
			if not linha:find("\tok$") then
				falhas = falhas + 1
			end
		end
		LrDialogs.message(
			interrompida and "Régua interrompida" or "Régua exportada",
			string.format("%d casos em %s (%d com problema — ver casos.txt).", #relatorio, pasta, falhas),
			"info"
		)
	end)
end)

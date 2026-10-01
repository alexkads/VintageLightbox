--[[
A régua do VintageLightbox dentro do Lightroom Classic.

Exporta a mesma foto revelada de muitos jeitos — um slider por vez, cada
perfil criativo sozinho e cada predefinição do estúdio inteira — para o
`comparar_com_o_lightroom` medir o motor contra o do Lightroom.

Instalar: Arquivo > Gerenciador de plug-ins > Adicionar > esta pasta.
Usar: selecionar UMA cópia virtual e Arquivo > Extras de plug-in >
"Exportar a régua do VintageLightbox".
]]
return {
	LrSdkVersion = 10.0,
	LrSdkMinimumVersion = 6.0,
	LrToolkitIdentifier = "br.com.recordarfotos.vintagelightbox.calibracao",
	LrPluginName = "VintageLightbox — régua de calibração",
	-- Iniciar.lua fica de olho em `pedido.txt` (ver lá) desde a abertura.
	LrInitPlugin = "Iniciar.lua",
	LrForceInitPlugin = true,
	LrExportMenuItems = {
		{ title = "Exportar a régua do VintageLightbox", file = "Calibrar.lua" },
	},
	LrLibraryMenuItems = {
		{ title = "Exportar a régua do VintageLightbox", file = "Calibrar.lua" },
	},
	VERSION = { major = 1, minor = 0, revision = 0, build = 0 },
}

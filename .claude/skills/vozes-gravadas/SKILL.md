---
name: vozes-gravadas
description: Como criar ou mudar um aviso falado do VintageLightbox — as frases gravadas com o Kokoro 82M (voz pm_santa, a "voz natalina") pelo `ferramentas/gravador-de-vozes/regravar.sh`, comprimidas em Ogg Vorbis e embutidas em `crates/ui-gpui/assets/vozes/`, e como ligar uma frase a um evento do `sons` (aviso de chatbot, agenda, conexão, alerta da API). Usar quando o dono pedir um aviso sonoro ou falado novo, mudar o texto de uma voz, "gravar uma frase", "Kokoro", "voz do Papai Noel/pm_santa", ou quando um aviso precisar falar algo que hoje só toca som.
---

# Vozes gravadas dos avisos

> Dono, 15/11/2026: *"Eu fico com VintageLightBox aberto em minha máquina, então posso receber
> notificações urgentes de problemas na API usando Kokoro TTS"*, *"use a voz lúdica do Natal"*,
> *"esses áudios teria que ser num formato bem comprimido"* e *"use o áudio dessas mensagens ao
> máximo"*. E, para esta skill: *"para ficar o conhecimento para a próxima vez que precisarmos de
> algum tipo de aviso"*.

## A decisão: gravar antes, não falar na hora

O Kokoro 82M fala português do Brasil com naturalidade, mas para falar **na hora** cada balcão
precisaria baixar 329 MB de modelo, carregar até 1,7 GB de memória e compilar o candle a cada
atualização. As frases de aviso são poucas e fixas, então **cada uma é gravada uma vez**, na máquina
de quem grava, e embutida no app (`include_bytes!`). O balcão só toca um arquivo de ~15 KB.

É o mesmo padrão da locução dos treinamentos do Raven Finanças (RavenRS, issue #607: "gravar
antes"). Consequências:

- funciona **sem rede** — o "servidor não está respondendo" toca justamente quando o servidor caiu;
- soa igual no Mac, no Windows e no Linux;
- texto variável (nome do cliente, valor) **não** cabe numa gravação: ou a frase é genérica ("Um
  cliente está pedindo um atendente"), ou o evento fica com a voz do sistema (`sons/voz.rs`).

## Onde está cada coisa

| Peça | Onde |
|---|---|
| Os textos | `crates/ui-gpui/assets/vozes/frases.tsv` (`nome<TAB>texto`, um por linha) |
| Os áudios | `crates/ui-gpui/assets/vozes/<nome>.ogg` (Vorbis mono 24 kHz, qualidade −0,1) |
| Embutir | `crates/ui-gpui/src/sons/gravadas.rs` (macro `gravadas!`: `Variante => "arquivo.ogg"`) |
| Evento → frase | `Evento::gravada()` em `crates/ui-gpui/src/sons/mod.rs` |
| Tocar | `Som::Gravada` em `sons/porta.rs` (o mesmo `rodio::Decoder` dos sons próprios) |
| Gravar | `ferramentas/gravador-de-vozes/` (fora do workspace) e `regravar.sh` |
| O motor | `~/Projects/RavenRS/crates/ravenrs-ai` (`SpeechEngine`, Kokoro em candle, fonemas pt-BR próprios) |

## Receita: um aviso falado novo

1. **Escreva o texto** em `frases.tsv`. Regras que já morderam:
   - **sem números** — o Kokoro ainda não os lê (RavenRS #96): escreva por extenso ou evite;
   - nomes compostos **separados**: "Recordar Fotos", e não "RecordarFotos" (o fonema sai errado);
   - termine com **o que fazer**, quando houver: *"Pode continuar trabalhando: eu guardo tudo e
     envio quando ele voltar."*;
   - a voz é lúdica e toca em balcões onde o cliente ouve: alerta técnico vai só a administradores.
2. **Grave**:
   ```bash
   ferramentas/gravador-de-vozes/regravar.sh            # todas
   ferramentas/gravador-de-vozes/regravar.sh nova.tsv   # só as de um arquivo (mesmo formato)
   ```
   Na primeira vez ele baixa o modelo (327 MB, hexgrad/Kokoro-82M na versão fixa do catálogo do
   RavenRS, cada arquivo conferido por SHA-256) em `~/.cache/vintagelightbox/modelos` e compila o
   gravador (~2 min). Depois, ~2 s por frase. Precisa do RavenRS em `~/Projects/RavenRS`.
3. **Ouça** antes de embutir: `afplay ~/.cache/vintagelightbox/vozes-para-ouvir/<nome>.wav`.
   🚨 **Nunca confira com `afplay` no `.ogg`**: o macOS toca Ogg Vorbis **pela metade** (2,5 s de
   4,4 s em 15/11). O `.wav` de ouvir é o `.ogg` já decodificado — o som real da compressão.
4. **Embuta**: uma linha na macro de `sons/gravadas.rs`.
5. **Ligue a um evento**: se o aviso é novo, um `Evento` em `sons/mod.rs` (`TODOS`, `chave` — que
   nunca muda —, `rotulo`, `padrao`, `exemplo`, `gravada`) e quem o dispara chama
   `crate::sons::soar(Evento::X, titulo, detalhe, cx)`. Com modo de voz e gravação, toca a gravação.
6. **Teste**: `cargo test -p ui-gpui --lib -- sons::gravadas` abre cada `.ogg` pelo `rodio` e confere
   a duração (1 a 10 s, o teto de um aviso) e a linha em `frases.tsv`.

## Os números (15/11/2026, Apple Silicon)

| Etapa | Valor |
|---|---|
| Carregar o modelo | 0,6 s |
| Gerar | ~2× mais rápido que a fala (5,4 s de fala em 2,6 s) |
| Memória no pico, só durante a gravação | 1,3 GB |
| WAV de 4,4 s | 212 KB |
| Ogg Vorbis q 0 / **−0,1** / −0,2 | 19 / **15** / 14 KB |
| As 17 frases de 15/11 | 241 KB no binário |

Por que Vorbis: o `rodio` do app já o toca (feature `vorbis`), sem dependência nova; o Opus seria um
pouco menor, mas pediria outro decodificador em todo balcão. A série do fluxo Ogg vem do nome da
frase: regravar o mesmo texto gera o mesmo arquivo, e o git só mostra o que mudou.

## O que não fazer

- ❌ Kokoro **dentro do app** (falar na hora): 329 MB por balcão, 1,7 GB de pico e o candle no
  `cargo build` de toda atualização. Só se um dia o texto precisar ser livre — e aí atrás de uma
  feature desligada nos balcões.
- ❌ Pôr o gravador no workspace: os balcões o compilariam ao atualizar.
- ❌ Conferir com `afplay` no `.ogg` (ver acima).
- ❌ Frase com número, sigla colada ou nome de cliente.

## As frases de hoje

`frases.tsv` é a lista viva. Em 15/11/2026: os 9 avisos que já existiam (mensagem no chatbot, cliente
no QR, agendamento novo e cancelado, falha, fotos salvas, exportação, nova versão, cartão ejetado),
**cliente pediu atendente**, os três **alertas da API** (falha, pouca memória, voltou ao normal — só
a administradores, pelo fluxo SSE do app; issue #22 do e-commerce) e a **conexão** (servidor fora,
sem internet, voltou, tudo enviado — decididos pelo próprio app em `app/vozes.rs`, com um teste de
3 s a `1.1.1.1`/`8.8.8.8` para separar "sem internet" de "servidor fora").

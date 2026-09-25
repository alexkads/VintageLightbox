#!/usr/bin/env python3
"""Regressoes dos instaladores, sem rede, compilacao ou instalacao de dependencias.

Execute: python3 scripts/testar-instalador.py

Cobre os dois arquivos:
  instalar-vintagelightbox-gpui.cmd   o app GPUI (crates/ui-gpui)
  instalar-vintagelightbox.cmd        o endereco antigo, que despacha para ele

PowerShell e validado se pwsh estiver disponivel; cmd.exe exige Windows real.
"""

import io
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest


PASTA = Path(__file__).resolve().parent
ANTIGO = PASTA / "instalar-vintagelightbox.cmd"

APPS = {
    "gpui": {
        "script": PASTA / "instalar-vintagelightbox-gpui.cmd",
        "crate": "ui-gpui",
        "bin": "ui-gpui",
        "app_mac": "VintageLightbox (Zed GPUI).app",
        "identificador": "br.com.recordarfotos.vintagelightbox",
    },
}
TODOS = [ANTIGO] + [a["script"] for a in APPS.values()]


class Ambiente(unittest.TestCase):
    """Uma casa, um PATH de mentira e um tarball do repositorio."""

    app = "gpui"

    def setUp(self):
        self.info = APPS[self.app]
        self.temp = tempfile.TemporaryDirectory(prefix="vlb-teste-")
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.bin = self.base / "bin"
        self.bin.mkdir()
        self.casa = self.base / "cache"
        self.fonte = self.casa / f"fonte-{self.app}"
        self.destino = self.base / "Aplicativos"
        self.home = self.base / "home"
        self.home.mkdir()
        self.log = self.base / "cargo.log"
        self.archive = self.base / "fonte.tar.gz"
        self.sem_xcode = self.base / "sem-xcode"
        self.sem_xcode.mkdir()
        self.env = dict(os.environ, HOME=str(self.home), VLB_CASA=str(self.casa),
                        PATH=f"{self.bin}:/usr/bin:/bin:/usr/sbin:/sbin",
                        TMPDIR=str(self.base), TEST_ARCHIVE=str(self.archive),
                        TEST_LOG=str(self.log), LIBCLANG_PATH=str(self.bin),
                        VLB_PASTA_XCODE=str(self.sem_xcode))
        self.env.pop("VLB_APP", None)
        self.env.pop("XDG_CURRENT_DESKTOP", None)
        # Nem o Fedora imutável nem as extensões do GNOME desta máquina entram.
        self.env["VLB_OSTREE"] = str(self.base / "sem-ostree")
        self.env["VLB_OS_RELEASE"] = str(self.base / "sem-os-release")
        self.extensoes = self.base / "extensoes-gnome"
        self.extensoes.mkdir()
        self.env["VLB_EXTENSOES_GNOME"] = str(self.extensoes)
        self.env.pop("VLB_SECO", None)
        (self.bin / "libclang.so").touch()
        self.mock("uname", "echo Darwin")
        self.mock("xcrun", "echo /usr/bin/clang++")
        self.mock("xcode-select", "echo /Developer")
        self.mock("xcodebuild", "exit 0")
        self.mock("sudo", 'echo "sudo chamado: $*" >&2; exit 99')
        self.mock("rustc", "echo 'rustc 1.98.0 (teste)'")
        self.mock("pkg-config", "echo 4.1")
        # O instalador Linux também confere o suporte PTP/GVfs antes de
        # compilar; no ambiente falso, esses dois comandos representam as
        # ferramentas já instaladas.
        self.mock("gphoto2", "echo gphoto2")
        self.mock("gio", "echo gio")
        self.env["VLB_PTP_BACKEND_OK"] = "1"
        # O `gdbus` de verdade falaria com o GNOME Shell de quem roda o teste.
        self.mock("gdbus", "exit 1")
        self.mock("codesign", "exit 0")
        self.mock("ditto", 'cp -R "$1" "$2"')
        # O raw.githubusercontent.com (os instaladores) sai de TEST_RAW_DIR, e
        # falha sem ele; o resto (o tarball do codigo) e o TEST_ARCHIVE.
        self.mock("curl", '''
url=""; out=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) out="$2"; shift 2 ;;
    http*) url="$1"; shift ;;
    *) shift ;;
  esac
done
case "$url" in
  *raw.githubusercontent.com*)
    [ -n "${TEST_RAW_DIR:-}" ] || exit 22
    cp "$TEST_RAW_DIR/${url##*/}" "$out"
    exit 0 ;;
esac
[ -z "$out" ] || cp "$TEST_ARCHIVE" "$out"
exit "${TEST_CURL_EXIT:-0}"
''')
        self.mock("cargo", '''
printf '%s\\n' "$*" >> "$TEST_LOG"
[ "${TEST_BUILD_EXIT:-0}" -eq 0 ] || exit "$TEST_BUILD_EXIT"
bin=""; manifesto=""; perfil=release
while [ "$#" -gt 0 ]; do
  case "$1" in
    --profile) perfil="$2"; shift 2 ;;
    --bin) bin="$2"; shift 2 ;;
    --manifest-path) manifesto="$2"; shift 2 ;;
    *) shift ;;
  esac
done
fonte="$(dirname "$manifesto")"
mkdir -p "$CARGO_TARGET_DIR/$perfil"
# O binário falso responde `--versao` como o de verdade (a prova de vida que o
# instalador pede antes de trocar o instalado). TEST_VERSAO_EXIT simula um
# binário novo que não abre.
printf '#!/bin/sh\\nif [ "$1" = "--versao" ]; then [ "${TEST_VERSAO_EXIT:-0}" -eq 0 ] || exit "$TEST_VERSAO_EXIT"; echo 9.9.9; fi\\nexit 0\\n' > "$CARGO_TARGET_DIR/$perfil/$bin"
chmod +x "$CARGO_TARGET_DIR/$perfil/$bin"
[ -f "$fonte/Cargo.lock" ] || echo '# lock local' > "$fonte/Cargo.lock"
''')
        self.files = {
            "Cargo.toml": '[workspace.package]\nversion = "0.1.8"\n',
            "Cargo.lock": "# lock de teste\n",
            "crates/ui-gpui/Cargo.toml": '[package]\nname = "ui-gpui"\n',
            "empacotamento/icones/icone.icns": "icone",
            "empacotamento/icones/256x256.png": "icone",
        }
        self.archive_files()

    def mock(self, name, body):
        file = self.bin / name
        file.write_text("#!/bin/sh\nset -eu\n" + body + "\n")
        file.chmod(0o755)

    def archive_files(self):
        with tarfile.open(self.archive, "w:gz") as archive:
            for name, content in self.files.items():
                data = content.encode()
                info = tarfile.TarInfo("repo/" + name)
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))

    def run_script(self, script, *args, piped=False, destino=True):
        command = ["/bin/sh", "-s", "--"] if piped else ["/bin/sh", str(script)]
        extra = ["--destino", str(self.destino)] if destino else []
        return subprocess.run(
            command + extra + list(args), env=self.env,
            input=script.read_text() if piped else None, text=True,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=20,
        )

    def run_installer(self, *args, piped=False):
        return self.run_script(self.info["script"], *args, piped=piped)

    def app_mac(self):
        return self.destino / self.info["app_mac"]


class CasosDoInstalador:
    """Os mesmos casos para os dois apps (as subclasses escolhem `app`)."""

    def old_source(self):
        self.fonte.mkdir(parents=True)
        (self.fonte / "anterior").write_text("preservar")

    def assert_preserved(self, result):
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertEqual((self.fonte / "anterior").read_text(), "preservar")
        self.assertFalse(self.log.exists())
        self.assertEqual(list(self.casa.glob("download.*")), [])

    def test_download_com_erro_mesmo_com_arquivo_completo(self):
        self.old_source()
        self.env["TEST_CURL_EXIT"] = "18"
        self.assert_preserved(self.run_installer())

    def test_arquivo_corrompido(self):
        self.old_source()
        self.archive.write_text("incompleto")
        self.assert_preserved(self.run_installer())

    def test_versao_sem_app(self):
        self.old_source()
        del self.files[f"crates/{self.info['crate']}/Cargo.toml"]
        self.archive_files()
        result = self.run_installer()
        self.assert_preserved(result)
        self.assertIn(f"crates/{self.info['crate']}", result.stdout)

    def test_versao_sem_lock_preserva_resolucao_local(self):
        del self.files["Cargo.lock"]
        self.archive_files()
        first = self.run_installer()
        self.assertEqual(first.returncode, 0, first.stdout)
        lock = self.fonte / "Cargo.lock"
        lock.write_text("# resolucao local anterior")
        second = self.run_installer()
        self.assertEqual(second.returncode, 0, second.stdout)
        self.assertEqual(lock.read_text(), "# resolucao local anterior")
        self.assertIn("código sem alterações", second.stdout)

    def test_reinstalacao_preserva_datas_com_arquivos_gerados(self):
        first = self.run_installer(piped=True)
        self.assertEqual(first.returncode, 0, first.stdout)
        manifest = self.fonte / "Cargo.toml"
        os.utime(manifest, (1234567890, 1234567890))
        second = self.run_installer()
        self.assertEqual(second.returncode, 0, second.stdout)
        self.assertEqual(manifest.stat().st_mtime, 1234567890)
        self.assertTrue((self.app_mac() / "Contents/MacOS" / self.info["bin"]).exists())
        self.assertEqual(list(self.casa.glob("download.*")), [])

    def test_compila_so_o_binario_do_app(self):
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stdout)
        linha = self.log.read_text()
        self.assertIn(f"-p {self.info['crate']} --bin {self.info['bin']}", linha)
        self.assertIn("--profile instalador", linha)
        self.assertTrue((self.casa / f"target-{self.app}/instalador" / self.info["bin"]).exists())

    def test_info_plist_do_app(self):
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stdout)
        plist = (self.app_mac() / "Contents/Info.plist").read_text()
        self.assertIn(f"<string>{self.info['identificador']}</string>", plist)
        self.assertIn(f"<string>{self.info['bin']}</string>", plist)
        self.assertIn("<string>0.1.8</string>", plist)
        self.assertNotIn("#@", plist)

    def test_codigo_alterado_e_substituido(self):
        self.old_source()
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertFalse((self.fonte / "anterior").exists())
        self.assertEqual((self.fonte / "Cargo.toml").read_text(), self.files["Cargo.toml"])

    def test_build_falha_preserva_app_instalado(self):
        app = self.app_mac()
        app.mkdir(parents=True)
        (app / "anterior").write_text("preservar")
        self.env["TEST_BUILD_EXIT"] = "42"
        result = self.run_installer()
        self.assertEqual(result.returncode, 42, result.stdout)
        self.assertEqual((app / "anterior").read_text(), "preservar")
        self.assertIn("CARGO_BUILD_JOBS=1 sh", result.stdout)

    def test_compila_uma_por_4_gib_de_memoria(self):
        self.mock("uname", "echo Linux")
        self.mock("sysctl", "echo 8589934592")
        self.mock("getconf", "echo 8")
        self.mock("cargo", 'printf "jobs=%s\\n" "${CARGO_BUILD_JOBS:-}" >> "$TEST_LOG"; exit 42')
        result = self.run_installer()
        if Path("/proc/meminfo").exists():
            self.skipTest("a conta usa o /proc/meminfo desta máquina")
        self.assertIn("compilando 2 de cada vez", result.stdout)
        self.assertIn("jobs=2", self.log.read_text())

    def test_cargo_build_jobs_definido_vale_mais(self):
        self.mock("uname", "echo Linux")
        self.mock("sysctl", "echo 8589934592")
        self.mock("getconf", "echo 8")
        self.mock("cargo", 'printf "jobs=%s\\n" "${CARGO_BUILD_JOBS:-}" >> "$TEST_LOG"; exit 42')
        self.env["CARGO_BUILD_JOBS"] = "5"
        result = self.run_installer()
        self.assertNotIn("GiB de memória", result.stdout)
        self.assertIn("jobs=5", self.log.read_text())

    def test_a_falha_deixa_o_registro_e_diz_qual_mandar(self):
        # 🧾 O dono no Fedora 44: "é melhor atualizar o script de instalação
        # do Linux pra eu trazer informação". O código do cargo continua sendo
        # o da saída, e o registro tem o retrato e a saída da compilação.
        self.mock("uname", "echo Linux")
        self.env["TEST_BUILD_EXIT"] = "42"
        result = self.run_installer()
        self.assertEqual(result.returncode, 42, result.stdout)
        self.assertIn("parou em: compilando", result.stdout)
        self.assertIn("Mande este arquivo", result.stdout)
        registros = list((self.casa / "registros").glob("instalacao-*.log"))
        self.assertEqual(len(registros), 1, result.stdout)
        self.assertIn(str(registros[0]), result.stdout)
        texto = registros[0].read_text()
        self.assertIn("retrato da máquina", texto)
        self.assertIn("rustc 1.98.0 (teste)", texto)
        self.assertIn("a compilação falhou", texto, "a saída de erro também vai")

    def test_o_sucesso_tambem_diz_onde_esta_o_registro(self):
        self.mock("uname", "echo Linux")
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("registro desta instalação:", result.stdout)
        self.assertNotIn("Mande este arquivo", result.stdout)

    def test_diagnostico_so_tira_o_retrato(self):
        self.mock("uname", "echo Linux")
        self.mock("cargo", "echo 'cargo nao deveria rodar' >&2; exit 99")
        result = self.run_installer("--diagnostico")
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("retrato da máquina", result.stdout)
        self.assertIn("rustc 1.98.0 (teste)", result.stdout)
        self.assertNotIn("nao deveria rodar", result.stdout)
        self.assertFalse(self.casa.exists(), "o retrato não grava nada")

    def test_seco_nao_grava(self):
        result = self.run_installer("--seco")
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertFalse(self.casa.exists())
        self.assertFalse(self.destino.exists())
        self.assertFalse(self.log.exists())

    def test_linux_instala_binario_e_atalho(self):
        self.mock("uname", "echo Linux")
        result = self.run_installer(piped=True)
        self.assertEqual(result.returncode, 0, result.stdout)
        nome = f"vintagelightbox-{self.app}"
        self.assertTrue((self.destino / "bin" / nome).exists())
        atalho = (self.destino / f"share/applications/{nome}.desktop").read_text()
        self.assertIn(f"Exec={self.destino}/bin/{nome}", atalho)
        self.assertIn(f"Icon={nome}", atalho)
        self.assertNotIn("#@", atalho)

    def test_linux_troca_sem_corromper_e_guarda_o_anterior(self):
        """O novo entra por renomeação e o instalado fica como `.anterior`."""
        self.mock("uname", "echo Linux")
        nome = f"vintagelightbox-{self.app}"
        instalado = self.destino / "bin" / nome
        instalado.parent.mkdir(parents=True)
        instalado.write_text("o app que funciona\n")
        result = self.run_installer(piped=True)
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("o app novo responde a versão 9.9.9", result.stdout)
        self.assertIn("--versao", instalado.read_text(), "o novo está no lugar")
        anterior = self.destino / "bin" / f"{nome}.anterior"
        self.assertEqual(anterior.read_text(), "o app que funciona\n", "o anterior ficou guardado")
        self.assertFalse((self.destino / "bin" / f".{nome}.novo").exists(), "sem resto da troca")

    def test_linux_binario_novo_que_nao_abre_nao_toca_no_instalado(self):
        """🔑 O que funciona só é trocado depois de o novo provar que abre."""
        self.mock("uname", "echo Linux")
        self.env["TEST_VERSAO_EXIT"] = "3"
        nome = f"vintagelightbox-{self.app}"
        instalado = self.destino / "bin" / nome
        instalado.parent.mkdir(parents=True)
        instalado.write_text("o app que funciona\n")
        result = self.run_installer(piped=True)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("o app novo não abriu (--versao); o instalado continua como estava", result.stdout)
        self.assertEqual(instalado.read_text(), "o app que funciona\n", "intacto")
        self.assertFalse((self.destino / "bin" / f"{nome}.anterior").exists())

    def test_macos_troca_sem_corromper_e_guarda_o_anterior(self):
        """No macOS o `.app` inteiro vira `.anterior`, e o novo entra por `mv`."""
        app = self.app_mac()
        (app / "Contents/MacOS").mkdir(parents=True)
        (app / "Contents/MacOS/marca").write_text("o app que funciona\n")
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stdout)
        anterior = app.parent / f"{app.name}.anterior"
        self.assertEqual((anterior / "Contents/MacOS/marca").read_text(), "o app que funciona\n")
        self.assertFalse((app / "Contents/MacOS/marca").exists(), "o novo está no lugar")
        self.assertFalse((app.parent / f".{app.name}.novo").exists(), "sem resto da troca")

    def test_macos_binario_novo_que_nao_abre_nao_toca_no_instalado(self):
        self.env["TEST_VERSAO_EXIT"] = "3"
        app = self.app_mac()
        (app / "Contents/MacOS").mkdir(parents=True)
        (app / "Contents/MacOS/marca").write_text("o app que funciona\n")
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertEqual((app / "Contents/MacOS/marca").read_text(), "o app que funciona\n")

    def test_linux_seco_lista_pacotes_que_faltam(self):
        self.mock("uname", "echo Linux")
        self.mock("pkg-config", "exit 1")
        self.mock("apt-get", "echo 'apt-get nao deveria rodar' >&2; exit 99")
        result = self.run_installer("--seco")
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("apt-get install -y", result.stdout)
        self.assertIn("[seco] não instalei nada", result.stdout)
        self.assertNotIn("nao deveria rodar", result.stdout)

    def linux_sem_pacotes(self, distro):
        self.mock("uname", "echo Linux")
        self.mock("pkg-config", "exit 1")
        self.mock("apt-get", "exit 0")
        self.mock("dnf", "exit 0")
        os_release = self.base / "os-release"
        os_release.write_text(distro)
        self.env["VLB_OS_RELEASE"] = str(os_release)
        return self.run_installer("--seco")

    def test_fedora_com_apt_get_usa_o_dnf(self):
        result = self.linux_sem_pacotes('NAME="Fedora Linux"\nID=fedora\n')
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("dnf install -y", result.stdout)
        self.assertNotIn("apt-get", result.stdout)

    def test_ubuntu_com_dnf_usa_o_apt(self):
        result = self.linux_sem_pacotes('ID=ubuntu\nID_LIKE=debian\n')
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("apt-get install -y", result.stdout)
        self.assertNotIn("dnf install", result.stdout)

    def test_fedora_imutavel_mostra_o_rpm_ostree(self):
        self.mock("uname", "echo Linux")
        self.mock("pkg-config", "exit 1")
        self.mock("dnf", "echo 'dnf nao deveria rodar' >&2; exit 99")
        ostree = self.base / "ostree-booted"
        ostree.touch()
        self.env["VLB_OSTREE"] = str(ostree)
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("sudo rpm-ostree install --idempotent", result.stdout)
        self.assertNotIn("nao deveria rodar", result.stdout)
        self.assertFalse(self.log.exists())

    def test_gnome_no_fedora_instala_a_extensao_da_bandeja(self):
        self.mock("uname", "echo Linux")
        self.mock("sudo", '"$@"')
        self.mock("dnf", 'printf "dnf %s\\n" "$*" >> "$TEST_LOG"')
        self.env["XDG_CURRENT_DESKTOP"] = "GNOME"
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("dnf install -y gnome-shell-extension-appindicator",
                      self.log.read_text())

    def test_gnome_com_a_extensao_ligada_nao_mexe(self):
        self.mock("uname", "echo Linux")
        self.mock("dnf", "echo 'dnf nao deveria rodar' >&2; exit 99")
        self.mock("gnome-extensions", "echo '  Enabled: Yes'")
        (self.extensoes / "appindicatorsupport@rgcjonas.gmail.com").mkdir()
        self.env["XDG_CURRENT_DESKTOP"] = "ubuntu:GNOME"
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("extensão da bandeja do GNOME: ligada", result.stdout)
        self.assertNotIn("nao deveria rodar", result.stdout)

    def test_gnome_sem_sessao_manda_ligar_a_extensao_a_mao(self):
        self.mock("uname", "echo Linux")
        self.mock("gnome-extensions", "exit 1")
        # Sem sessão gráfica o dconf também não responde.
        self.mock("gsettings", "exit 1")
        (self.extensoes / "appindicatorsupport@rgcjonas.gmail.com").mkdir()
        self.env["XDG_CURRENT_DESKTOP"] = "GNOME"
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("Abra o app Extensões", result.stdout)
        self.assertNotIn("extensão da bandeja ligada", result.stdout)

    def test_gnome_recem_instalada_liga_pelo_gsettings(self):
        # Fedora 44: o `enable` falha porque o Shell ainda não conhece a
        # extensão que o `dnf` acabou de instalar; a lista do dconf, não.
        self.mock("uname", "echo Linux")
        self.mock("gnome-extensions", "exit 1")
        self.mock(
            "gsettings",
            'if [ "$1" = get ]; then echo "[\'outra@exemplo.com\']"; '
            'else printf "gsettings %s\\n" "$*" >> "$TEST_LOG"; fi',
        )
        (self.extensoes / "appindicatorsupport@rgcjonas.gmail.com").mkdir()
        self.env["XDG_CURRENT_DESKTOP"] = "GNOME"
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("saia e entre de novo", result.stdout)
        self.assertNotIn("não consegui ligar", result.stdout)
        self.assertIn(
            "enabled-extensions ['outra@exemplo.com', "
            "'appindicatorsupport@rgcjonas.gmail.com']",
            self.log.read_text(),
        )

    def test_gnome_recem_instalada_carrega_sem_sair_da_sessao(self):
        # O Shell carrega na hora o que ele mesmo instala: depois do `gdbus`
        # (que responde erro mesmo dando certo), a extensão está ACTIVE.
        self.mock("uname", "echo Linux")
        carregada = self.base / "carregada"
        self.env["TEST_CARREGADA"] = str(carregada)
        self.mock(
            "gnome-extensions",
            '[ "$1" = info ] && [ -e "$TEST_CARREGADA" ] || exit 1\n'
            "echo '  State: ACTIVE'",
        )
        self.mock("gsettings", 'if [ "$1" = get ]; then echo "@as []"; fi')
        self.mock(
            "gdbus",
            'printf "gdbus %s\\n" "$*" >> "$TEST_LOG"; touch "$TEST_CARREGADA"; exit 1',
        )
        (self.extensoes / "appindicatorsupport@rgcjonas.gmail.com").mkdir()
        self.env["XDG_CURRENT_DESKTOP"] = "GNOME"
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("ligada, sem sair da sessão", result.stdout)
        self.assertNotIn("saia e entre de novo", result.stdout)
        self.assertIn("InstallRemoteExtension appindicatorsupport@rgcjonas.gmail.com",
                      self.log.read_text())

    def test_diagnostico_diz_que_o_shell_nao_conhece_a_extensao(self):
        self.mock("uname", "echo Linux")
        self.mock("gnome-shell", "echo 'GNOME Shell 50.5'")
        self.mock("gnome-extensions", "echo 'A extensão não existe' >&2; exit 2")
        (self.extensoes / "appindicatorsupport@rgcjonas.gmail.com").mkdir()
        result = self.run_installer("--diagnostico")
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("no disco, mas o Shell desta sessão não a conhece", result.stdout)
        self.assertRegex(result.stdout, r"rustup: +—")

    def test_ajuda_aponta_o_instalador_do_app(self):
        result = self.run_installer("--ajuda", piped=True)
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn(f"instalar-vintagelightbox-{self.app}.cmd | sh", result.stdout)


class InstaladorGpui(CasosDoInstalador, Ambiente):
    app = "gpui"

    def test_macos_sem_xcode_compila_com_shaders_em_tempo_de_execucao(self):
        self.mock("xcrun", '[ "$2" = metal ] && exit 1; echo /usr/bin/clang++')
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertNotIn("Xcode e Metal", result.stdout)
        self.assertNotIn("sudo chamado", result.stdout)
        self.assertIn("--features shaders-em-tempo-de-execucao", self.log.read_text())
        self.assertTrue(self.app_mac().exists())

    def test_macos_sem_command_line_tools(self):
        self.mock("xcrun", "exit 1")
        result = self.run_installer()
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("xcode-select --install", result.stdout)
        self.assertFalse(self.log.exists())

    def test_linux_nao_liga_a_feature_do_macos(self):
        self.mock("uname", "echo Linux")
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertNotIn("--features", self.log.read_text())

    def test_nao_apaga_o_app_antigo_do_dmg(self):
        antigo = self.destino / "VintageLightbox.app"
        antigo.mkdir(parents=True)
        (antigo / "anterior").write_text("preservar")
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual((antigo / "anterior").read_text(), "preservar")
        self.assertTrue(self.app_mac().exists())


class EnderecoAntigo(Ambiente):
    """O instalar-vintagelightbox.cmd despacha para o instalador do app."""

    def test_despacha_para_o_instalador_do_app(self):
        # Rodado de um arquivo, sem internet: usa a copia ao lado.
        result = self.run_script(ANTIGO)
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertTrue((self.destino / "VintageLightbox (Zed GPUI).app/Contents/MacOS/ui-gpui").exists())
        self.assertIn("-p ui-gpui --bin ui-gpui", self.log.read_text())

    def test_curl_pipe_sh_baixa_o_instalador_e_repassa_opcoes(self):
        self.env["TEST_RAW_DIR"] = str(PASTA)
        result = self.run_script(ANTIGO, "--seco", piped=True)
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("instalar-vintagelightbox-gpui.cmd", result.stdout)
        self.assertIn("[seco]", result.stdout)
        self.assertFalse(self.log.exists())
        self.assertEqual(list(self.base.glob("vintagelightbox.*")), [])

    def test_curl_pipe_sh_sem_internet_falha_claro(self):
        result = self.run_script(ANTIGO, piped=True)
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("não consegui baixar", result.stdout)
        self.assertFalse(self.log.exists())
        self.assertEqual(list(self.base.glob("vintagelightbox.*")), [])

    def test_pagina_de_erro_nao_roda(self):
        raw = self.base / "raw"
        raw.mkdir()
        (raw / "instalar-vintagelightbox-gpui.cmd").write_text("<html>404</html>\n")
        self.env["TEST_RAW_DIR"] = str(raw)
        result = self.run_script(ANTIGO, piped=True)
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertFalse(self.log.exists())

    def test_falha_do_instalador_chega_ao_codigo_de_saida(self):
        self.env["TEST_BUILD_EXIT"] = "42"
        result = self.run_script(ANTIGO)
        self.assertEqual(result.returncode, 42, result.stdout)


class Estrutura(unittest.TestCase):
    """As marcas que fazem um arquivo so rodar em cmd, sh e PowerShell."""

    def test_gerados_batem_com_o_modelo(self):
        result = subprocess.run([sys.executable, str(PASTA / "gerar-instaladores.py"), "--conferir"],
                                capture_output=True, text=True, timeout=20)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_marcas_intactas(self):
        for script in TODOS:
            with self.subTest(script=script.name):
                bruto = script.read_bytes()
                self.assertNotIn(b"\r", bruto, "CRLF quebra o sh")
                linhas = bruto.decode("utf-8").split("\n")
                self.assertEqual(linhas[0], ':<<"::FIM-DO-CMD"')
                ordem = [linhas.index(m) for m in (
                    "::FIM-DO-CMD", ": <<'#==FIM-POWERSHELL=='", "#==POWERSHELL==",
                    "#==FIM-POWERSHELL==")]
                self.assertEqual(ordem, sorted(ordem))
                for marca in ("::FIM-DO-CMD", "#==POWERSHELL==", "#==FIM-POWERSHELL=="):
                    self.assertEqual(linhas.count(marca), 1, marca)
                cmd = linhas[:ordem[0]]
                self.assertIn("exit /b %VLB_RESULTADO%", cmd)
                # Dois cliques esperam uma tecla no fim; o app, que roda o
                # instalador sem janela, pede `VLB_SEM_PAUSA=1` — senão o
                # processo ficaria preso no `pause` para sempre.
                if script.name == APPS["gpui"]["script"].name:
                    self.assertIn('if not "%VLB_SEM_PAUSA%"=="1" pause', cmd)
                else:
                    # O despachante do endereço antigo: o app nunca o roda.
                    self.assertIn("pause", cmd)
                # O cmd baixa a versao mais nova DESTE arquivo, e nao de outro.
                propria = f"/main/scripts/{script.name}'"
                self.assertTrue(any(propria in l for l in cmd if l.startswith("powershell ")), propria)
                self.assertNotIn("@@", "\n".join(linhas))
                self.assertFalse(any(l.startswith("#@") for l in linhas))

    def test_sintaxe_sh(self):
        for script in TODOS:
            for shell in ("/bin/sh", shutil.which("dash")):
                if not shell:
                    continue
                with self.subTest(script=script.name, shell=shell):
                    result = subprocess.run([shell, "-n", str(script)], capture_output=True, text=True)
                    self.assertEqual(result.returncode, 0, result.stderr)

    @unittest.skipUnless(shutil.which("shellcheck"), "shellcheck indisponivel")
    def test_shellcheck(self):
        for script in TODOS:
            with self.subTest(script=script.name):
                result = subprocess.run(["shellcheck", "-s", "sh", str(script)],
                                        capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stdout)


@unittest.skipUnless(shutil.which("pwsh"), "PowerShell indisponivel")
class PowerShell(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="vlb-ps-")
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)

    def pwsh(self, codigo, *args, env=None):
        harness = self.base / "conferir.ps1"
        harness.write_text(codigo)
        return subprocess.run([shutil.which("pwsh"), "-NoProfile", "-File", str(harness), *map(str, args)],
                              capture_output=True, text=True, timeout=60, env=env)

    def test_sintaxe_dos_tres_blocos(self):
        codigo = r'''
$ErrorActionPreference = 'Stop'
foreach ($arquivo in $args) {
    $texto = [IO.File]::ReadAllText($arquivo)
    $inicio = $texto.IndexOf("`n#==POWERSHELL==")
    $fim = $texto.IndexOf("`n#==FIM-POWERSHELL==", $inicio)
    if ($inicio -lt 0 -or $fim -le $inicio) { throw "marcas: $arquivo" }
    $tokens = $null; $erros = $null
    [void][System.Management.Automation.Language.Parser]::ParseInput(
        $texto.Substring($inicio, $fim - $inicio), [ref]$tokens, [ref]$erros)
    if ($erros.Count) { throw "$arquivo`n" + ($erros | Out-String) }
    # O `exit` fecharia a janela sem o operador ler o erro.
    $saidas = $tokens | Where-Object { $_.Kind -eq 'Exit' }
    if ($saidas) { throw "exit dentro do bloco: $arquivo" }
}
'''
        result = self.pwsh(codigo, *TODOS)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def troca_do_windows(self, preparar):
        """Roda, no PowerShell de verdade, o trecho exato da troca do instalador
        gerado: do `$exe = Join-Path $Destino` até antes do ícone."""
        texto = APPS["gpui"]["script"].read_text()
        inicio = texto.index('$exe = Join-Path $Destino')
        fim = texto.index('Copy-Item (Join-Path $Fonte "empacotamento\\icones\\icone.ico")', inicio)
        destino = self.base / "Programs"
        mingw = self.base / "mingw"
        destino.mkdir()
        mingw.mkdir()
        (destino / "VintageLightbox-GPUI.exe").write_text("o que funciona")
        (destino / "libgcc_s_seh-1.dll").write_text("dll velha")
        (mingw / "libgcc_s_seh-1.dll").write_text("dll nova")
        binario = self.base / "novo.exe"
        binario.write_text("o novo")
        preparar(binario, destino, mingw)
        codigo = (
            "$ErrorActionPreference = 'Stop'\n"
            f"$Destino = '{destino}'\n$mingw = '{mingw}'\n$binario = '{binario}'\n"
            + texto[inicio:fim]
        )
        return self.pwsh(codigo), destino

    def test_troca_do_windows_guarda_o_anterior(self):
        """🔑 O instalado vira `.anterior` e o novo entra — exe e DLLs."""
        result, destino = self.troca_do_windows(lambda *_: None)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual((destino / "VintageLightbox-GPUI.exe").read_text(), "o novo")
        self.assertEqual((destino / "VintageLightbox-GPUI.exe.anterior").read_text(), "o que funciona")
        self.assertEqual((destino / "libgcc_s_seh-1.dll").read_text(), "dll nova")
        self.assertEqual((destino / "libgcc_s_seh-1.dll.anterior").read_text(), "dll velha")

    def test_troca_do_windows_que_falha_devolve_o_anterior(self):
        """🔑 Uma cópia que falha no meio devolve o que funcionava."""
        def sem_binario(binario, *_):
            binario.unlink()
        result, destino = self.troca_do_windows(sem_binario)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("a troca falhou e a versao anterior foi devolvida", result.stdout + result.stderr)
        self.assertEqual((destino / "VintageLightbox-GPUI.exe").read_text(), "o que funciona")
        self.assertFalse((destino / "VintageLightbox-GPUI.exe.anterior").exists())
        self.assertEqual((destino / "libgcc_s_seh-1.dll").read_text(), "dll velha", "a DLL nem foi tocada")

    def test_funcoes_do_powershell(self):
        # Analise o bloco real e execute suas funcoes com executaveis reais.
        codigo = r'''
$ErrorActionPreference = 'Stop'
$texto = [IO.File]::ReadAllText($args[0])
$inicio = $texto.IndexOf("`n#==POWERSHELL==")
$fim = $texto.IndexOf("`n#==FIM-POWERSHELL==", $inicio)
$tokens = $null; $erros = $null
$ast = [System.Management.Automation.Language.Parser]::ParseInput(
    $texto.Substring($inicio, $fim - $inicio), [ref]$tokens, [ref]$erros)
if ($erros.Count) { throw ($erros | Out-String) }
$funcao = $ast.Find({ param($n)
    $n -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq 'Correr'
}, $true)
Invoke-Expression $funcao.Extent.Text
$Seco = $false
$falhou = $false
try { Correr { & /bin/sh -c 'exit 23' } } catch { $falhou = $_ -match 'codigo 23' }
if (-not $falhou) { throw 'engoliu erro nativo' }
Correr { & /bin/sh -c 'exit 0' }
$Seco = $true
Correr { throw 'modo seco executou o comando' }
$funcao = $ast.Find({ param($n)
    $n -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq 'Assinatura-Fonte'
}, $true)
Invoke-Expression $funcao.Extent.Text
$pasta = Join-Path $PSScriptRoot 'fonte'
New-Item -ItemType Directory -Path "$pasta/crates/ui-gpui" -Force | Out-Null
Set-Content "$pasta/Cargo.toml" 'original'
$antes = @(Assinatura-Fonte $pasta)
if (Compare-Object $antes @(Assinatura-Fonte $pasta)) { throw 'fonte parada mudou de assinatura' }
Set-Content "$pasta/crates/ui-gpui/Cargo.toml" 'novo'
if (-not (Compare-Object $antes @(Assinatura-Fonte $pasta))) { throw 'ignorou arquivo novo' }
$antes = @(Assinatura-Fonte $pasta)
Set-Content "$pasta/Cargo.toml" 'alterado'
if (-not (Compare-Object $antes @(Assinatura-Fonte $pasta))) { throw 'ignorou fonte alterada' }
'''
        result = self.pwsh(codigo, APPS["gpui"]["script"])
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_g_mais_mais_e_testado_antes_de_compilar(self):
        codigo = r'''
$ErrorActionPreference = 'Stop'
$texto = [IO.File]::ReadAllText($args[0])
$inicio = $texto.IndexOf("`n#==POWERSHELL==")
$fim = $texto.IndexOf("`n#==FIM-POWERSHELL==", $inicio)
$tokens = $null; $erros = $null
$ast = [System.Management.Automation.Language.Parser]::ParseInput(
    $texto.Substring($inicio, $fim - $inicio), [ref]$tokens, [ref]$erros)
if ($erros.Count) { throw ($erros | Out-String) }
$funcao = $ast.Find({ param($n)
    $n -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq 'Testar-Gpp'
}, $true)
Invoke-Expression $funcao.Extent.Text
$bin = Join-Path $PSScriptRoot 'bin'
New-Item -ItemType Directory -Path $bin -Force | Out-Null
Set-Content "$bin/g++" "#!/bin/sh`necho 'cc1plus: fatal error: libisl-23.dll nao encontrada' >&2`nexit 1"
& chmod +x "$bin/g++"
$caminho = $env:PATH
$env:PATH = "$bin" + [IO.Path]::PathSeparator + $caminho
$r = Testar-Gpp
if ($r.Ok) { throw 'aceitou g++ quebrado' }
if ($r.Saida -notmatch 'libisl') { throw "sem a mensagem do compilador: $($r.Saida)" }
if ($ErrorActionPreference -ne 'Stop') { throw 'mudou a preferencia de erro de quem chamou' }
Set-Content "$bin/g++" "#!/bin/sh`nexit 0"
if (-not (Testar-Gpp).Ok) { throw 'recusou g++ que funciona' }
$env:PATH = $caminho
'''
        for app in APPS:
            with self.subTest(app=app):
                result = self.pwsh(codigo, APPS[app]["script"])
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_fxc_do_gpui(self):
        codigo = r'''
$ErrorActionPreference = 'Stop'
$texto = [IO.File]::ReadAllText($args[0])
$inicio = $texto.IndexOf("`n#==POWERSHELL==")
$fim = $texto.IndexOf("`n#==FIM-POWERSHELL==", $inicio)
$tokens = $null; $erros = $null
$ast = [System.Management.Automation.Language.Parser]::ParseInput(
    $texto.Substring($inicio, $fim - $inicio), [ref]$tokens, [ref]$erros)
$funcao = $ast.Find({ param($n)
    $n -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq 'Achar-Fxc'
}, $true)
Invoke-Expression $funcao.Extent.Text
${env:ProgramFiles(x86)} = Join-Path $PSScriptRoot 'x86'
$env:GPUI_FXC_PATH = $null
if (Achar-Fxc) { throw 'achou fxc inexistente' }
$sdk = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits/10/bin/10.0.26100.0/x64'
New-Item -ItemType Directory -Force -Path $sdk | Out-Null
Set-Content (Join-Path $sdk 'fxc.exe') 'x'
# O separador do Mac e '/', e o filtro procura '\x64\': o caminho vem de GPUI_FXC_PATH.
$env:GPUI_FXC_PATH = Join-Path $sdk 'fxc.exe'
if ((Achar-Fxc) -ne $env:GPUI_FXC_PATH) { throw 'ignorou GPUI_FXC_PATH' }
'''
        result = self.pwsh(codigo, APPS["gpui"]["script"])
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def seco(self, script, **extra):
        """Roda o trecho do cmd (a mesma linha) em modo seco, sem Windows."""
        linha = next(l for l in script.read_text().split("\n") if l.startswith("powershell "))
        comando = linha.split('-Command "', 1)[1].rsplit('"', 1)[0]
        vazio = self.base / "vazio"
        vazio.mkdir(exist_ok=True)
        env = dict(os.environ, VLB_SECO="1", VLB_SCRIPT=str(script), PATH=str(vazio),
                   USERPROFILE=str(self.base), LOCALAPPDATA=str(self.base),
                   ProgramFiles=str(self.base), TEMP=str(self.base), **extra)
        env.pop("VLB_APP", None)
        env.update(extra)
        env.pop("GPUI_FXC_PATH", None)
        env.pop("LIBCLANG_PATH", None)
        # O Mac nao tem a unidade C:, onde o instalador procura o MSYS2.
        comando = f"New-PSDrive -Name C -PSProvider FileSystem -Root '{vazio}' | Out-Null; " + comando
        return subprocess.run([shutil.which("pwsh"), "-NoProfile", "-Command", comando],
                              capture_output=True, text=True, timeout=60, env=env)

    def test_seco_de_ponta_a_ponta(self):
        casos = [
            (APPS["gpui"]["script"], {}, "-p ui-gpui --bin ui-gpui"),
            (ANTIGO, {}, "-p ui-gpui --bin ui-gpui"),
        ]
        for script, extra, esperado in casos:
            with self.subTest(script=script.name, **extra):
                result = self.seco(script, **extra)
                saida = result.stdout + result.stderr
                self.assertEqual(result.returncode, 0, saida)
                self.assertIn(esperado, saida)
                self.assertIn("[seco] nada foi feito.", saida)
                self.assertFalse((self.base / ".vintagelightbox").exists())


if __name__ == "__main__":
    unittest.main(verbosity=2)

//! Os arquivos que a interface desenha: os ícones do site e as imagens da capa.
//!
//! # Os ícones são os do site
//!
//! O site desenha com o `lucide-react`; `icones/` tem os mesmos desenhos em
//! SVG (licença ISC, em `icones/LICENSE`), gerados do pacote do site. Até
//! 2026-09-17 este app não registrava fonte de arquivos nenhuma, e por isso os
//! ícones que o próprio `gpui-component` pede (a seta do seletor, o `x` do
//! campo de busca) saíam vazios. Eles estão aqui com os nomes que ele procura
//! (`icons/close.svg`…).

use std::borrow::Cow;

use gpui_kit::component::IconNamed;
use gpui_kit::{AssetSource, SharedString};

/// `(caminho, bytes)`, gerado pelo `build.rs`.
static ARQUIVOS: &[(&str, &[u8])] = include!(concat!(env!("OUT_DIR"), "/recursos.rs"));

/// A fonte de arquivos do app, para `Application::with_assets`.
pub struct Recursos;

impl AssetSource for Recursos {
    fn load(&self, caminho: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        Ok(ARQUIVOS
            .iter()
            .find(|(nome, _)| *nome == caminho)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, pasta: &str) -> gpui_kit::Result<Vec<SharedString>> {
        Ok(ARQUIVOS
            .iter()
            .filter(|(nome, _)| nome.starts_with(pasta))
            .map(|(nome, _)| SharedString::from(*nome))
            .collect())
    }
}

/// Os bytes de uma imagem embutida (`imagens/…`).
pub fn imagem(nome: &str) -> Option<&'static [u8]> {
    let caminho = format!("imagens/{nome}");
    ARQUIVOS
        .iter()
        .find(|(n, _)| *n == caminho)
        .map(|(_, bytes)| *bytes)
}

/// 🔤 Os `.ttf` que o template embutiu (`fontes/<slug>/…`, ver `tema::fontes`).
pub fn fontes() -> impl Iterator<Item = &'static [u8]> {
    ARQUIVOS
        .iter()
        .filter(|(nome, _)| nome.starts_with("fontes/"))
        .map(|(_, bytes)| *bytes)
}

/// Os ícones que as telas do app usam, com o nome do lucide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icone {
    ArrowLeft,
    ArrowRight,
    // As camadas do editor sobem e descem na pilha.
    ArrowUp,
    ArrowDown,
    // A ferramenta Mover do editor (V).
    Move,
    // Desfoque e nitidez do editor, e o painel Histórico.
    Droplet,
    Triangle,
    History,
    Building2,
    Calculator,
    Camera,
    ChartColumn,
    Undo2,
    Save,
    Copy,
    MonitorOff,
    Undo,
    FlipVertical,
    FlipHorizontal,
    RotateCw,
    RotateCcw,
    Redo2,
    Download,
    Crop,
    Check,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    ChevronsUpDown,
    CircleAlert,
    ClipboardList,
    Cloud,
    Columns3,
    ExternalLink,
    FolderInput,
    GripVertical,
    /// 💛 A favorita apagada (contorno) e acesa (cheia) — o `Heart` do site,
    /// que pinta o mesmo desenho com `fill`.
    Heart,
    HeartCheio,
    HardDrive,
    Info,
    Keyboard,
    Link2,
    LoaderCircle,
    LogOut,
    Maximize2,
    Minus,
    Minimize2,
    Monitor,
    Moon,
    PanelLeft,
    PanelRightClose,
    PanelRightOpen,
    Pencil,
    Plus,
    Printer,
    Search,
    Send,
    ShoppingCart,
    SlidersHorizontal,
    Settings,
    Square,
    SquareCheck,
    Store,
    Sun,
    Trash2,
    TriangleAlert,
    Upload,
    UserPlus,
    X,
    ZoomIn,
    Map,
    ZoomOut,
    CalendarCheck,
    CircleCheck,
    Handshake,
    ImagePlus,
    ShoppingBag,
    Ticket,
    MonitorSmartphone,
    // 📦 A tela do backup (`/dashboard/backup`): a pasta, o arquivo e o
    // recarregar. Os três SVG já estavam em `icones/`.
    FolderOpen,
    File,
    RefreshCw,
    // 💬📅 O chatbot e a agenda no desktop (2026-09-25).
    MessageCircle,
    MessageSquare,
    Bot,
    Hand,
    Bell,
    BellOff,
    Globe,
    Zap,
    EllipsisVertical,
    Inbox,
    Calendar,
    CalendarDays,
    Clock,
    MapPin,
    Phone,
    // Os quatro da barra de janela, desenhados como os do Zed no Linux —
    // traço fino de 16 px, e não o lucide do resto do app (`crate::janela`).
    // 🟩 O tema Matrix no menu da conta.
    SquareTerminal,
    JanelaMinimizar,
    JanelaMaximizar,
    JanelaRestaurar,
    JanelaFechar,
    // A Revelação local.
    Paintbrush,
    Lasso,
    LassoSelect,
    Stamp,
    Bandage,
    Sparkles,
    Eye,
    EyeOff,
    Contrast,
    CirclePlus,
    CircleMinus,
    Layers,
    CircleDashed,
    RectangleHorizontal,
    Spline,
    Pentagon,
    Scan,
    // O QR do bot da sessão no painel do atendente.
    QrCode,
    // O Enquadrar.
    Ruler,
    /// O conta-gotas do balanço de branco (painel Básico).
    Pipette,
}

impl Icone {
    pub const TODOS: &'static [Icone] = &[
        Icone::ArrowLeft,
        Icone::ArrowRight,
        Icone::ArrowUp,
        Icone::ArrowDown,
        Icone::Move,
        Icone::Droplet,
        Icone::Triangle,
        Icone::History,
        Icone::Building2,
        Icone::Calculator,
        Icone::Camera,
        Icone::ChartColumn,
        Icone::Undo2,
        Icone::Save,
        Icone::Copy,
        Icone::MonitorOff,
        Icone::Undo,
        Icone::FlipVertical,
        Icone::FlipHorizontal,
        Icone::RotateCw,
        Icone::RotateCcw,
        Icone::Redo2,
        Icone::Download,
        Icone::Crop,
        Icone::Check,
        Icone::ChevronDown,
        Icone::ChevronLeft,
        Icone::ChevronRight,
        Icone::ChevronsUpDown,
        Icone::CircleAlert,
        Icone::ClipboardList,
        Icone::Cloud,
        Icone::Columns3,
        Icone::ExternalLink,
        Icone::FolderInput,
        Icone::GripVertical,
        Icone::Heart,
        Icone::HeartCheio,
        Icone::HardDrive,
        Icone::Info,
        Icone::Keyboard,
        Icone::Link2,
        Icone::LoaderCircle,
        Icone::LogOut,
        Icone::Maximize2,
        Icone::Minus,
        Icone::Minimize2,
        Icone::Monitor,
        Icone::Moon,
        Icone::PanelLeft,
        Icone::PanelRightClose,
        Icone::PanelRightOpen,
        Icone::Pencil,
        Icone::Plus,
        Icone::Printer,
        Icone::Search,
        Icone::Send,
        Icone::ShoppingCart,
        Icone::SlidersHorizontal,
        Icone::Settings,
        Icone::Square,
        Icone::SquareCheck,
        Icone::Store,
        Icone::Sun,
        Icone::Trash2,
        Icone::TriangleAlert,
        Icone::Upload,
        Icone::UserPlus,
        Icone::X,
        Icone::ZoomIn,
        Icone::Map,
        Icone::ZoomOut,
        Icone::CalendarCheck,
        Icone::CircleCheck,
        Icone::Handshake,
        Icone::ImagePlus,
        Icone::ShoppingBag,
        Icone::Ticket,
        Icone::MonitorSmartphone,
        Icone::FolderOpen,
        Icone::File,
        Icone::RefreshCw,
        Icone::MessageCircle,
        Icone::MessageSquare,
        Icone::Bot,
        Icone::Hand,
        Icone::Bell,
        Icone::BellOff,
        Icone::Globe,
        Icone::Zap,
        Icone::EllipsisVertical,
        Icone::Inbox,
        Icone::Calendar,
        Icone::CalendarDays,
        Icone::Clock,
        Icone::MapPin,
        Icone::Phone,
        Icone::SquareTerminal,
        Icone::JanelaMinimizar,
        Icone::JanelaMaximizar,
        Icone::JanelaRestaurar,
        Icone::JanelaFechar,
        Icone::Paintbrush,
        Icone::Lasso,
        Icone::LassoSelect,
        Icone::Stamp,
        Icone::Bandage,
        Icone::Sparkles,
        Icone::Eye,
        Icone::EyeOff,
        Icone::Contrast,
        Icone::CirclePlus,
        Icone::CircleMinus,
        Icone::Layers,
        Icone::CircleDashed,
        Icone::RectangleHorizontal,
        Icone::Spline,
        Icone::Pentagon,
        Icone::Scan,
        Icone::QrCode,
        Icone::Ruler,
        Icone::Pipette,
    ];

    fn arquivo(self) -> &'static str {
        match self {
            Icone::ArrowLeft => "arrow-left",
            Icone::ArrowRight => "arrow-right",
            Icone::ArrowUp => "arrow-up",
            Icone::ArrowDown => "arrow-down",
            Icone::Move => "move",
            Icone::Droplet => "droplet",
            Icone::Triangle => "triangle",
            Icone::History => "history",
            Icone::Building2 => "building-2",
            Icone::Calculator => "calculator",
            Icone::Camera => "camera",
            Icone::ChartColumn => "chart-column",
            Icone::Undo2 => "undo-2",
            Icone::Save => "save",
            Icone::Copy => "copy",
            Icone::MonitorOff => "monitor-off",
            Icone::Undo => "undo",
            Icone::FlipVertical => "flip-vertical",
            Icone::FlipHorizontal => "flip-horizontal",
            Icone::RotateCw => "rotate-cw",
            Icone::RotateCcw => "rotate-ccw",
            Icone::Redo2 => "redo-2",
            Icone::Download => "download",
            Icone::Crop => "crop",
            Icone::Check => "check",
            Icone::ChevronDown => "chevron-down",
            Icone::ChevronLeft => "chevron-left",
            Icone::ChevronRight => "chevron-right",
            Icone::ChevronsUpDown => "chevrons-up-down",
            Icone::CircleAlert => "circle-alert",
            Icone::ClipboardList => "clipboard-list",
            Icone::Cloud => "cloud",
            Icone::Columns3 => "columns-3",
            Icone::ExternalLink => "external-link",
            Icone::FolderInput => "folder-input",
            Icone::GripVertical => "grip-vertical",
            Icone::Heart => "heart",
            Icone::HeartCheio => "heart-cheio",
            Icone::HardDrive => "hard-drive",
            Icone::Info => "info",
            Icone::Keyboard => "keyboard",
            Icone::Link2 => "link-2",
            Icone::LoaderCircle => "loader-circle",
            Icone::LogOut => "log-out",
            Icone::Maximize2 => "maximize-2",
            Icone::Minus => "minus",
            Icone::Minimize2 => "minimize-2",
            Icone::Monitor => "monitor",
            Icone::Moon => "moon",
            Icone::PanelLeft => "panel-left",
            Icone::PanelRightClose => "panel-right-close",
            Icone::PanelRightOpen => "panel-right-open",
            Icone::Pencil => "pencil",
            Icone::Plus => "plus",
            Icone::Printer => "printer",
            Icone::Search => "search",
            Icone::Send => "send",
            Icone::ShoppingCart => "shopping-cart",
            Icone::SlidersHorizontal => "sliders-horizontal",
            Icone::Settings => "settings",
            Icone::Square => "square",
            Icone::SquareCheck => "square-check",
            Icone::Store => "store",
            Icone::Sun => "sun",
            Icone::Trash2 => "trash-2",
            Icone::FolderOpen => "folder-open",
            Icone::File => "file",
            Icone::RefreshCw => "refresh-cw",
            Icone::TriangleAlert => "triangle-alert",
            Icone::Upload => "upload",
            Icone::UserPlus => "user-plus",
            Icone::X => "x",
            Icone::ZoomIn => "zoom-in",
            Icone::Map => "map",
            Icone::ZoomOut => "zoom-out",
            Icone::CalendarCheck => "calendar-check",
            Icone::CircleCheck => "circle-check",
            Icone::Handshake => "handshake",
            Icone::ImagePlus => "image-plus",
            Icone::ShoppingBag => "shopping-bag",
            Icone::Ticket => "ticket",
            Icone::MonitorSmartphone => "monitor-smartphone",
            Icone::MessageCircle => "message-circle",
            Icone::MessageSquare => "message-square",
            Icone::Bot => "bot",
            Icone::Hand => "hand",
            Icone::Bell => "bell",
            Icone::BellOff => "bell-off",
            Icone::Globe => "globe",
            Icone::Zap => "zap",
            Icone::EllipsisVertical => "ellipsis-vertical",
            Icone::Inbox => "inbox",
            Icone::Calendar => "calendar",
            Icone::CalendarDays => "calendar-days",
            Icone::Clock => "clock",
            Icone::MapPin => "map-pin",
            Icone::Phone => "phone",
            Icone::SquareTerminal => "square-terminal",
            Icone::JanelaMinimizar => "janela-minimizar",
            Icone::JanelaMaximizar => "janela-maximizar",
            Icone::JanelaRestaurar => "janela-restaurar",
            Icone::JanelaFechar => "janela-fechar",
            Icone::Paintbrush => "paintbrush",
            Icone::Lasso => "lasso",
            Icone::LassoSelect => "lasso-select",
            Icone::Stamp => "stamp",
            Icone::Bandage => "bandage",
            Icone::Sparkles => "sparkles",
            Icone::Eye => "eye",
            Icone::EyeOff => "eye-off",
            Icone::Contrast => "contrast",
            Icone::CirclePlus => "circle-plus",
            Icone::CircleMinus => "circle-minus",
            Icone::Layers => "layers",
            Icone::CircleDashed => "circle-dashed",
            Icone::RectangleHorizontal => "rectangle-horizontal",
            Icone::Spline => "spline",
            Icone::Pentagon => "pentagon",
            Icone::Scan => "scan",
            Icone::QrCode => "qr-code",
            Icone::Ruler => "ruler",
            Icone::Pipette => "pipette",
        }
    }
}

impl IconNamed for Icone {
    fn path(self) -> SharedString {
        format!("icons/{}.svg", self.arquivo()).into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn todo_icone_do_app_existe() {
        for icone in Icone::TODOS {
            let caminho = icone.path();
            assert!(
                Recursos.load(&caminho).unwrap().is_some(),
                "{caminho} não está em icones/ — a tela mostraria um ícone vazio"
            );
        }
    }

    /// Os que o `gpui-component` procura pelo nome dele.
    #[test]
    fn os_icones_do_gpui_component_existem() {
        use gpui_kit::component::IconName;
        for icone in [
            IconName::Close,
            IconName::ChevronDown,
            IconName::Check,
            IconName::Search,
            IconName::Minus,
            IconName::Plus,
            IconName::ArrowUp,
            IconName::ArrowDown,
            IconName::Loader,
            IconName::Info,
            IconName::TriangleAlert,
            IconName::CircleCheck,
            IconName::CircleX,
        ] {
            let caminho = icone.path();
            assert!(Recursos.load(&caminho).unwrap().is_some(), "{caminho}");
        }
    }

    #[test]
    fn as_imagens_da_capa_estao_embutidas() {
        assert!(imagem("capa-canela.jpeg").is_some());
        assert!(imagem("selo.png").is_some());
    }

    /// 🎨 A foto das amostras de preset é colorida: numa sépia ou P&B, "Preto
    /// e branco", "Sépia" e "RecordarFotos P&B" saem iguais (dono, 03/10/2026).
    #[test]
    fn a_foto_das_amostras_e_colorida() {
        let bytes = imagem("amostra-casal.jpeg").expect("embutida");
        let foto = image::load_from_memory(bytes).unwrap().to_rgb8();
        let coloridos = foto
            .pixels()
            .filter(|p| {
                let [r, g, b] = p.0.map(i32::from);
                (r - g).abs().max((g - b).abs()).max((r - b).abs()) > 60
            })
            .count();
        assert!(
            coloridos * 20 > foto.pixels().len(),
            "só {coloridos} pixels com cor"
        );
    }
}

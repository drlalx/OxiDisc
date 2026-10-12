/// Multi-language (i18n) localization module for OxiDisc.
/// Supports English (en), German (de), French (fr), Italian (it), Spanish (es),
/// Portuguese (pt), Japanese (ja), Russian (ru), Polish (pl), and Chinese (zh).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    En,
    De,
    Fr,
    It,
    Es,
    Pt,
    Ja,
    Ru,
    Pl,
    Zh,
}

#[allow(dead_code)]
impl Language {
    pub const ALL: [Language; 10] = [
        Language::En,
        Language::De,
        Language::Fr,
        Language::It,
        Language::Es,
        Language::Pt,
        Language::Ja,
        Language::Ru,
        Language::Pl,
        Language::Zh,
    ];

    pub fn as_code(&self) -> &'static str {
        match self {
            Self::En => "en",
            Self::De => "de",
            Self::Fr => "fr",
            Self::It => "it",
            Self::Es => "es",
            Self::Pt => "pt",
            Self::Ja => "ja",
            Self::Ru => "ru",
            Self::Pl => "pl",
            Self::Zh => "zh",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::En => "English",
            Self::De => "Deutsch",
            Self::Fr => "Français",
            Self::It => "Italiano",
            Self::Es => "Español",
            Self::Pt => "Português",
            Self::Ja => "日本語",
            Self::Ru => "Русский",
            Self::Pl => "Polski",
            Self::Zh => "中文",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        let code = code.trim().to_lowercase();
        if code.starts_with("de") {
            Some(Self::De)
        } else if code.starts_with("fr") {
            Some(Self::Fr)
        } else if code.starts_with("it") {
            Some(Self::It)
        } else if code.starts_with("es") {
            Some(Self::Es)
        } else if code.starts_with("pt") {
            Some(Self::Pt)
        } else if code.starts_with("ja") {
            Some(Self::Ja)
        } else if code.starts_with("ru") {
            Some(Self::Ru)
        } else if code.starts_with("pl") {
            Some(Self::Pl)
        } else if code.starts_with("zh") {
            Some(Self::Zh)
        } else if code.starts_with("en") {
            Some(Self::En)
        } else {
            None
        }
    }

    pub fn detect_system_language() -> Self {
        let env_lang = std::env::var("LC_ALL")
            .or_else(|_| std::env::var("LC_MESSAGES"))
            .or_else(|_| std::env::var("LANG"))
            .unwrap_or_default();

        Self::from_code(&env_lang).unwrap_or(Self::En)
    }

    pub fn resolve(config_val: &str) -> Self {
        let trimmed = config_val.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto") {
            Self::detect_system_language()
        } else {
            Self::from_code(trimmed).unwrap_or_else(Self::detect_system_language)
        }
    }

    // --- Header & Navigation ---

    pub fn tab_rip(&self) -> &'static str {
        match self {
            Self::En => "Rip CD",
            Self::De => "CD rippen",
            Self::Fr => "Extraire CD",
            Self::It => "Estrai CD",
            Self::Es => "Extraer CD",
            Self::Pt => "Extrair CD",
            Self::Ja => "CD取り込み",
            Self::Ru => "Копировать CD",
            Self::Pl => "Zgraj CD",
            Self::Zh => "抓取 CD",
        }
    }

    pub fn tab_settings(&self) -> &'static str {
        match self {
            Self::En => "Settings",
            Self::De => "Einstellungen",
            Self::Fr => "Paramètres",
            Self::It => "Impostazioni",
            Self::Es => "Configuración",
            Self::Pt => "Configurações",
            Self::Ja => "設定",
            Self::Ru => "Настройки",
            Self::Pl => "Ustawienia",
            Self::Zh => "设置",
        }
    }

    pub fn tab_help(&self) -> &'static str {
        match self {
            Self::En => "Help",
            Self::De => "Hilfe",
            Self::Fr => "Aide",
            Self::It => "Aiuto",
            Self::Es => "Ayuda",
            Self::Pt => "Ajuda",
            Self::Ja => "ヘルプ",
            Self::Ru => "Справка",
            Self::Pl => "Pomoc",
            Self::Zh => "帮助",
        }
    }

    pub fn drive(&self) -> &'static str {
        match self {
            Self::En => "Drive",
            Self::De => "Laufwerk",
            Self::Fr => "Lecteur",
            Self::It => "Unità",
            Self::Es => "Unidad",
            Self::Pt => "Unidade",
            Self::Ja => "ドライブ",
            Self::Ru => "Привод",
            Self::Pl => "Napęd",
            Self::Zh => "光驱",
        }
    }

    pub fn eject(&self) -> &'static str {
        match self {
            Self::En => "Eject",
            Self::De => "Auswerfen",
            Self::Fr => "Éjecter",
            Self::It => "Espelli",
            Self::Es => "Expulsar",
            Self::Pt => "Ejetar",
            Self::Ja => "取り出し",
            Self::Ru => "Извлечь",
            Self::Pl => "Wysuń",
            Self::Zh => "弹出",
        }
    }

    pub fn load_tray(&self) -> &'static str {
        match self {
            Self::En => "Load",
            Self::De => "Einziehen",
            Self::Fr => "Charger",
            Self::It => "Carica",
            Self::Es => "Cargar",
            Self::Pt => "Carregar",
            Self::Ja => "トレイ読込",
            Self::Ru => "Загрузить",
            Self::Pl => "Wsuń",
            Self::Zh => "加载",
        }
    }

    pub fn refresh(&self) -> &'static str {
        match self {
            Self::En => "Refresh",
            Self::De => "Aktualisieren",
            Self::Fr => "Actualiser",
            Self::It => "Aggiorna",
            Self::Es => "Actualizar",
            Self::Pt => "Atualizar",
            Self::Ja => "更新",
            Self::Ru => "Обновить",
            Self::Pl => "Odśwież",
            Self::Zh => "刷新",
        }
    }

    // --- Disc Info Sidebar ---

    pub fn disc_info(&self) -> &'static str {
        match self {
            Self::En => "Disc Info",
            Self::De => "Disc-Info",
            Self::Fr => "Infos disque",
            Self::It => "Info disco",
            Self::Es => "Información del disco",
            Self::Pt => "Informações do disco",
            Self::Ja => "ディスク情報",
            Self::Ru => "Информация о диске",
            Self::Pl => "Informacje o płycie",
            Self::Zh => "光盘信息",
        }
    }

    pub fn unknown_album(&self) -> &'static str {
        match self {
            Self::En => "Unknown Album",
            Self::De => "Unbekanntes Album",
            Self::Fr => "Album inconnu",
            Self::It => "Album sconosciuto",
            Self::Es => "Álbum desconocido",
            Self::Pt => "Álbum desconhecido",
            Self::Ja => "不明なアルバム",
            Self::Ru => "Неизвестный альбом",
            Self::Pl => "Nieznany album",
            Self::Zh => "未知专辑",
        }
    }

    pub fn unknown_artist(&self) -> &'static str {
        match self {
            Self::En => "Unknown Artist",
            Self::De => "Unbekannter Künstler",
            Self::Fr => "Artiste inconnu",
            Self::It => "Artista sconosciuto",
            Self::Es => "Artista desconocido",
            Self::Pt => "Artista desconhecido",
            Self::Ja => "不明なアーティスト",
            Self::Ru => "Неизвестный исполнитель",
            Self::Pl => "Nieznany wykonawca",
            Self::Zh => "未知艺术家",
        }
    }

    pub fn tracks(&self) -> &'static str {
        match self {
            Self::En => "Tracks",
            Self::De => "Titel",
            Self::Fr => "Pistes",
            Self::It => "Tracce",
            Self::Es => "Pistas",
            Self::Pt => "Faixas",
            Self::Ja => "トラック",
            Self::Ru => "Треки",
            Self::Pl => "Ścieżki",
            Self::Zh => "曲目",
        }
    }

    pub fn min(&self) -> &'static str {
        match self {
            Self::En => "min",
            Self::De => "Min.",
            Self::Fr => "min",
            Self::It => "min",
            Self::Es => "min",
            Self::Pt => "min",
            Self::Ja => "分",
            Self::Ru => "мин",
            Self::Pl => "min",
            Self::Zh => "分钟",
        }
    }

    pub fn verified(&self) -> &'static str {
        match self {
            Self::En => "Verified",
            Self::De => "Verifiziert",
            Self::Fr => "Vérifié",
            Self::It => "Verificato",
            Self::Es => "Verificado",
            Self::Pt => "Verificado",
            Self::Ja => "検証済み",
            Self::Ru => "Проверено",
            Self::Pl => "Zweryfikowano",
            Self::Zh => "已验证",
        }
    }

    pub fn format_and_quality(&self) -> &'static str {
        match self {
            Self::En => "Format & Quality",
            Self::De => "Format & Qualität",
            Self::Fr => "Format & Qualité",
            Self::It => "Formato e qualità",
            Self::Es => "Formato y calidad",
            Self::Pt => "Formato e qualidade",
            Self::Ja => "フォーマット & 音質",
            Self::Ru => "Формат и качество",
            Self::Pl => "Format i jakość",
            Self::Zh => "格式与音质",
        }
    }

    pub fn output_dir(&self) -> &'static str {
        match self {
            Self::En => "Output Directory",
            Self::De => "Ausgabe-Ordner",
            Self::Fr => "Dossier de sortie",
            Self::It => "Cartella di destinazione",
            Self::Es => "Carpeta de salida",
            Self::Pt => "Pasta de saída",
            Self::Ja => "出力フォルダ",
            Self::Ru => "Папка сохранения",
            Self::Pl => "Katalog wyjściowy",
            Self::Zh => "输出目录",
        }
    }

    pub fn change(&self) -> &'static str {
        match self {
            Self::En => "Change",
            Self::De => "Ändern",
            Self::Fr => "Modifier",
            Self::It => "Modifica",
            Self::Es => "Cambiar",
            Self::Pt => "Alterar",
            Self::Ja => "変更",
            Self::Ru => "Изменить",
            Self::Pl => "Zmień",
            Self::Zh => "更改",
        }
    }

    pub fn start_ripping(&self) -> &'static str {
        match self {
            Self::En => "START RIPPING CD",
            Self::De => "CD RIPPEN STARTEN",
            Self::Fr => "DÉMARRER L'EXTRACTION",
            Self::It => "AVVIA ESTRAZIONE CD",
            Self::Es => "INICIAR EXTRACCIÓN",
            Self::Pt => "INICIAR EXTRAÇÃO CD",
            Self::Ja => "CD取り込み開始",
            Self::Ru => "НАЧАТЬ КОПИРОВАНИЕ",
            Self::Pl => "ROZPOCZNIJ ZGRYWANIE CD",
            Self::Zh => "开始抓取 CD",
        }
    }

    pub fn cancel_ripping(&self) -> &'static str {
        match self {
            Self::En => "CANCEL RIPPING",
            Self::De => "RIPPEN ABBRECHEN",
            Self::Fr => "ANNULER L'EXTRACTION",
            Self::It => "ANNULLA ESTRAZIONE",
            Self::Es => "CANCELAR EXTRACCIÓN",
            Self::Pt => "CANCELAR EXTRAÇÃO",
            Self::Ja => "取り込み中止",
            Self::Ru => "ОТМЕНИТЬ КОПИРОВАНИЕ",
            Self::Pl => "ANULUJ ZGRYWANIE",
            Self::Zh => "取消抓取",
        }
    }

    pub fn ripping_in_progress(&self) -> &'static str {
        match self {
            Self::En => "Ripping in progress...",
            Self::De => "Rippen läuft...",
            Self::Fr => "Extraction en cours...",
            Self::It => "Estrazione in corso...",
            Self::Es => "Extracción en curso...",
            Self::Pt => "Extração em andamento...",
            Self::Ja => "取り込み処理中...",
            Self::Ru => "Идет копирование...",
            Self::Pl => "Zgrywanie w toku...",
            Self::Zh => "正在抓取...",
        }
    }

    pub fn total_progress(&self) -> &'static str {
        match self {
            Self::En => "Total Progress",
            Self::De => "Gesamtfortschritt",
            Self::Fr => "Progression totale",
            Self::It => "Avanzamento totale",
            Self::Es => "Progreso total",
            Self::Pt => "Progresso total",
            Self::Ja => "全体の進捗",
            Self::Ru => "Общий прогресс",
            Self::Pl => "Całkowity postęp",
            Self::Zh => "总进度",
        }
    }

    pub fn current_track(&self) -> &'static str {
        match self {
            Self::En => "Current Track",
            Self::De => "Aktueller Track",
            Self::Fr => "Piste actuelle",
            Self::It => "Traccia corrente",
            Self::Es => "Pista actual",
            Self::Pt => "Faixa atual",
            Self::Ja => "現在のトラック",
            Self::Ru => "Текущий трек",
            Self::Pl => "Bieżąca ścieżka",
            Self::Zh => "当前曲目",
        }
    }

    pub fn elapsed(&self) -> &'static str {
        match self {
            Self::En => "Elapsed",
            Self::De => "Verstrichen",
            Self::Fr => "Écoulé",
            Self::It => "Trascorso",
            Self::Es => "Transcurrido",
            Self::Pt => "Decorrido",
            Self::Ja => "経過時間",
            Self::Ru => "Прошло",
            Self::Pl => "Upłynęło",
            Self::Zh => "已用时间",
        }
    }

    // --- Tracklist Table ---

    pub fn col_title(&self) -> &'static str {
        match self {
            Self::En => "Title",
            Self::De => "Titel",
            Self::Fr => "Titre",
            Self::It => "Titolo",
            Self::Es => "Título",
            Self::Pt => "Título",
            Self::Ja => "タイトル",
            Self::Ru => "Название",
            Self::Pl => "Tytuł",
            Self::Zh => "标题",
        }
    }

    pub fn col_duration(&self) -> &'static str {
        match self {
            Self::En => "Duration",
            Self::De => "Dauer",
            Self::Fr => "Durée",
            Self::It => "Durata",
            Self::Es => "Duración",
            Self::Pt => "Duração",
            Self::Ja => "時間",
            Self::Ru => "Длительность",
            Self::Pl => "Czas",
            Self::Zh => "时长",
        }
    }

    pub fn col_status(&self) -> &'static str {
        match self {
            Self::En => "Status",
            Self::De => "Status",
            Self::Fr => "Statut",
            Self::It => "Stato",
            Self::Es => "Estado",
            Self::Pt => "Estado",
            Self::Ja => "状態",
            Self::Ru => "Статус",
            Self::Pl => "Status",
            Self::Zh => "状态",
        }
    }

    pub fn status_ready(&self) -> &'static str {
        match self {
            Self::En => "Ready",
            Self::De => "Bereit",
            Self::Fr => "Prêt",
            Self::It => "Pronto",
            Self::Es => "Listo",
            Self::Pt => "Pronto",
            Self::Ja => "待機中",
            Self::Ru => "Готов",
            Self::Pl => "Gotowy",
            Self::Zh => "就绪",
        }
    }

    pub fn status_ripping(&self) -> &'static str {
        match self {
            Self::En => "Ripping",
            Self::De => "Rippe",
            Self::Fr => "Extraction",
            Self::It => "Estrazione",
            Self::Es => "Extrayendo",
            Self::Pt => "Extraindo",
            Self::Ja => "抽出中",
            Self::Ru => "Копирование",
            Self::Pl => "Zgrywanie",
            Self::Zh => "正在抓取",
        }
    }

    pub fn status_done(&self) -> &'static str {
        match self {
            Self::En => "Done",
            Self::De => "Fertig",
            Self::Fr => "Terminé",
            Self::It => "Completato",
            Self::Es => "Completado",
            Self::Pt => "Concluído",
            Self::Ja => "完了",
            Self::Ru => "Готово",
            Self::Pl => "Ukończono",
            Self::Zh => "已完成",
        }
    }

    pub fn status_skipped(&self) -> &'static str {
        match self {
            Self::En => "Skipped",
            Self::De => "Übersprungen",
            Self::Fr => "Ignoré",
            Self::It => "Saltato",
            Self::Es => "Omitido",
            Self::Pt => "Ignorado",
            Self::Ja => "スキップ",
            Self::Ru => "Пропущен",
            Self::Pl => "Pominięto",
            Self::Zh => "已跳过",
        }
    }

    pub fn status_failed(&self) -> &'static str {
        match self {
            Self::En => "Failed",
            Self::De => "Fehler",
            Self::Fr => "Échec",
            Self::It => "Errore",
            Self::Es => "Error",
            Self::Pt => "Falhou",
            Self::Ja => "失敗",
            Self::Ru => "Ошибка",
            Self::Pl => "Błąd",
            Self::Zh => "失败",
        }
    }

    pub fn select_all(&self) -> &'static str {
        match self {
            Self::En => "Select All",
            Self::De => "Alle auswählen",
            Self::Fr => "Tout sélectionner",
            Self::It => "Seleziona tutto",
            Self::Es => "Seleccionar todo",
            Self::Pt => "Selecionar tudo",
            Self::Ja => "すべて選択",
            Self::Ru => "Выбрать все",
            Self::Pl => "Zaznacz wszystko",
            Self::Zh => "全选",
        }
    }

    pub fn select_none(&self) -> &'static str {
        match self {
            Self::En => "Deselect All",
            Self::De => "Keine auswählen",
            Self::Fr => "Tout désélectionner",
            Self::It => "Deseleziona tutto",
            Self::Es => "Deseleccionar todo",
            Self::Pt => "Desmarcar tudo",
            Self::Ja => "選択を解除",
            Self::Ru => "Снять выбор",
            Self::Pl => "Odznacz wszystko",
            Self::Zh => "取消全选",
        }
    }

    pub fn no_cd_detected(&self) -> &'static str {
        match self {
            Self::En => "No Audio CD Detected",
            Self::De => "Keine Audio-CD erkannt",
            Self::Fr => "Aucun CD audio détecté",
            Self::It => "Nessun CD audio rilevato",
            Self::Es => "No se detectó ningún CD de audio",
            Self::Pt => "Nenhum CD de áudio detectado",
            Self::Ja => "音楽CDが検出されません",
            Self::Ru => "Аудиодиск не обнаружен",
            Self::Pl => "Nie wykryto płyty Audio CD",
            Self::Zh => "未检测到音频 CD",
        }
    }

    pub fn insert_cd_prompt(&self) -> &'static str {
        match self {
            Self::En => "Insert an Audio CD into the drive or launch in mock mode (--mock-cd).",
            Self::De => "Legen Sie eine Audio-CD ein oder starten Sie mit --mock-cd.",
            Self::Fr => "Insérez un CD audio dans le lecteur ou lancez avec --mock-cd.",
            Self::It => "Inserire un CD audio nell'unità o avviare in modalità mock (--mock-cd).",
            Self::Es => "Inserte un CD de audio en la unidad o inicie con --mock-cd.",
            Self::Pt => "Insira um CD de áudio na unidade ou inicie com --mock-cd.",
            Self::Ja => "ドライブに音楽CDを挿入するか、--mock-cd で起動してください。",
            Self::Ru => "Вставьте Audio CD в дисковод или запустите с флагом --mock-cd.",
            Self::Pl => "Włóż płytę Audio CD do napędu lub uruchom w trybie testowym (--mock-cd).",
            Self::Zh => "请将音频 CD 插入光驱，或以模拟模式 (--mock-cd) 启动。",
        }
    }

    pub fn reading_cd(&self) -> &'static str {
        match self {
            Self::En => "Reading Audio CD Table of Contents...",
            Self::De => "Lese Audio-CD Inhaltsverzeichnis...",
            Self::Fr => "Lecture de la table des matières du CD audio...",
            Self::It => "Lettura del sommario del CD audio...",
            Self::Es => "Leyendo la tabla de contenido del CD de audio...",
            Self::Pt => "A ler o índice do CD de áudio...",
            Self::Ja => "音楽CDの目次を読み込み中...",
            Self::Ru => "Чтение оглавления Audio CD...",
            Self::Pl => "Odczytywanie spisu utworów płyty Audio CD...",
            Self::Zh => "正在读取音频 CD 目录...",
        }
    }

    pub fn detected_tracks_with_provider(&self, count: usize, provider: &str) -> String {
        match self {
            Self::En => format!("Detected: {count} Tracks (Metadata via {provider})"),
            Self::De => format!("Erkannt: {count} Tracks (Metadaten via {provider})"),
            Self::Fr => format!("Détecté : {count} pistes (Métadonnées via {provider})"),
            Self::It => format!("Rilevato: {count} tracce (Metadati via {provider})"),
            Self::Es => format!("Detectado: {count} pistas (Metadatos vía {provider})"),
            Self::Pt => format!("Detetado: {count} faixas (Metadados via {provider})"),
            Self::Ja => format!("検出: {count} トラック ({provider} 経由のメタデータ)"),
            Self::Ru => format!("Обнаружено: {count} треков (метаданные через {provider})"),
            Self::Pl => format!("Wykryto: {count} ścieżek (Metadane przez {provider})"),
            Self::Zh => format!("检测到: {count} 首曲目 (元数据通过 {provider})"),
        }
    }

    pub fn detected_tracks(&self, count: usize) -> String {
        match self {
            Self::En => format!("Detected: {count} Tracks"),
            Self::De => format!("Erkannt: {count} Tracks"),
            Self::Fr => format!("Détecté : {count} pistes"),
            Self::It => format!("Rilevato: {count} tracce"),
            Self::Es => format!("Detectado: {count} pistas"),
            Self::Pt => format!("Detetado: {count} faixas"),
            Self::Ja => format!("検出: {count} トラック"),
            Self::Ru => format!("Обнаружено: {count} треков"),
            Self::Pl => format!("Wykryto: {count} ścieżek"),
            Self::Zh => format!("检测到: {count} 首曲目"),
        }
    }

    // --- Settings View ---

    pub fn settings_language(&self) -> &'static str {
        match self {
            Self::En => "Language:",
            Self::De => "Sprache:",
            Self::Fr => "Langue :",
            Self::It => "Lingua:",
            Self::Es => "Idioma:",
            Self::Pt => "Idioma:",
            Self::Ja => "言語:",
            Self::Ru => "Язык:",
            Self::Pl => "Język:",
            Self::Zh => "语言:",
        }
    }

    pub fn system_default(&self) -> &'static str {
        match self {
            Self::En => "System Default",
            Self::De => "System-Standard",
            Self::Fr => "Par défaut du système",
            Self::It => "Predefinito di sistema",
            Self::Es => "Predeterminado del sistema",
            Self::Pt => "Padrão do sistema",
            Self::Ja => "システム既定",
            Self::Ru => "Системный по умолчанию",
            Self::Pl => "Domyślny systemowy",
            Self::Zh => "系统默认",
        }
    }

    pub fn settings_interface(&self) -> &'static str {
        match self {
            Self::En => "Interface:",
            Self::De => "Benutzeroberfläche:",
            Self::Fr => "Interface :",
            Self::It => "Interfaccia:",
            Self::Es => "Interfaz:",
            Self::Pt => "Interface:",
            Self::Ja => "インターフェース:",
            Self::Ru => "Интерфейс:",
            Self::Pl => "Interfejs:",
            Self::Zh => "界面:",
        }
    }

    pub fn settings_restart_notice(&self) -> &'static str {
        match self {
            Self::En => "Interface changes take effect after restart.",
            Self::De => "Änderungen an der Oberfläche werden nach Neustart wirksam.",
            Self::Fr => "Les changements d'interface prennent effet après redémarrage.",
            Self::It => "Le modifiche all'interfaccia avranno effetto dopo il riavvio.",
            Self::Es => "Los cambios de interfaz surten efecto tras reiniciar.",
            Self::Pt => "As alterações na interface terão efeito após reiniciar.",
            Self::Ja => "インターフェースの変更は再起動後に有効になります。",
            Self::Ru => "Изменения интерфейса вступят в силу после перезапуска.",
            Self::Pl => "Zmiana interfejsu zacznie obowiązywać po ponownym uruchomieniu.",
            Self::Zh => "界面的更改将在重启后生效。",
        }
    }

    pub fn settings_metadata_source(&self) -> &'static str {
        match self {
            Self::En => "Metadata Source:",
            Self::De => "Metadaten-Quelle:",
            Self::Fr => "Source des métadonnées :",
            Self::It => "Sorgente metadati:",
            Self::Es => "Fuente de metadatos:",
            Self::Pt => "Fonte de metadados:",
            Self::Ja => "メタデータソース:",
            Self::Ru => "Источник метаданных:",
            Self::Pl => "Źródło metadanych:",
            Self::Zh => "元数据来源:",
        }
    }

    pub fn settings_cover_size(&self) -> &'static str {
        match self {
            Self::En => "Album Art Resolution:",
            Self::De => "Cover-Auflösung:",
            Self::Fr => "Résolution de la pochette :",
            Self::It => "Risoluzione copertina:",
            Self::Es => "Resolución de la portada:",
            Self::Pt => "Resolução da capa:",
            Self::Ja => "ジャケット画像解像度:",
            Self::Ru => "Разрешение обложки:",
            Self::Pl => "Rozdzielczość okładki:",
            Self::Zh => "封面分辨率:",
        }
    }

    pub fn settings_cover_behavior(&self) -> &'static str {
        match self {
            Self::En => "Cover Art Download:",
            Self::De => "Cover-Download Verhalten:",
            Self::Fr => "Téléchargement de la pochette :",
            Self::It => "Download copertina:",
            Self::Es => "Descarga de la portada:",
            Self::Pt => "Download da capa:",
            Self::Ja => "ジャケット画像保存:",
            Self::Ru => "Сохранение обложки:",
            Self::Pl => "Pobieranie okładki:",
            Self::Zh => "封面下载方式:",
        }
    }

    pub fn settings_cover_preview_only(&self) -> &'static str {
        match self {
            Self::En => "Preview only in application",
            Self::De => "Nur Vorschau in der App",
            Self::Fr => "Aperçu uniquement dans l'application",
            Self::It => "Solo anteprima nell'applicazione",
            Self::Es => "Solo vista previa en la aplicación",
            Self::Pt => "Apenas pré-visualização no aplicativo",
            Self::Ja => "アプリ内プレビューのみ",
            Self::Ru => "Только просмотр в приложении",
            Self::Pl => "Tylko podgląd w aplikacji",
            Self::Zh => "仅在程序内预览",
        }
    }

    pub fn settings_cover_save_with_rip(&self) -> &'static str {
        match self {
            Self::En => "Save cover.jpg alongside ripped tracks",
            Self::De => "cover.jpg im Albumordner mitspeichern",
            Self::Fr => "Enregistrer cover.jpg dans le dossier de l'album",
            Self::It => "Salva cover.jpg nella cartella dell'album",
            Self::Es => "Guardar cover.jpg en la carpeta del álbum",
            Self::Pt => "Salvar cover.jpg na pasta do álbum",
            Self::Ja => "アルバムフォルダに cover.jpg を保存",
            Self::Ru => "Сохранять cover.jpg в папку с альбомом",
            Self::Pl => "Zapisz cover.jpg w folderze z albumem",
            Self::Zh => "在专辑文件夹中保存 cover.jpg",
        }
    }

    pub fn settings_auto_create_dir(&self) -> &'static str {
        match self {
            Self::En => "Automatically create missing destination folder without asking",
            Self::De => "Fehlende Zielordner automatisch ohne Nachfrage erstellen",
            Self::Fr => "Créer automatiquement les dossiers manquants sans confirmation",
            Self::It => "Crea automaticamente le cartelle mancanti senza chiedere",
            Self::Es => "Crear automáticamente las carpetas inexistentes sin confirmación",
            Self::Pt => "Criar pastas inexistentes automaticamente sem perguntar",
            Self::Ja => "保存先フォルダが存在しない場合は確認なしで自動作成",
            Self::Ru => "Автоматически создавать отсутствующие папки без подтверждения",
            Self::Pl => "Automatycznie twórz brakujące foldery bez pytania",
            Self::Zh => "目标文件夹不存在时自动创建，无需询问",
        }
    }

    pub fn settings_save(&self) -> &'static str {
        match self {
            Self::En => "Save Settings",
            Self::De => "Einstellungen speichern",
            Self::Fr => "Enregistrer les paramètres",
            Self::It => "Salva impostazioni",
            Self::Es => "Guardar configuración",
            Self::Pt => "Salvar configurações",
            Self::Ja => "設定を保存",
            Self::Ru => "Сохранить настройки",
            Self::Pl => "Zapisz ustawienia",
            Self::Zh => "保存设置",
        }
    }

    pub fn settings_saved(&self) -> &'static str {
        match self {
            Self::En => "Settings saved successfully!",
            Self::De => "Einstellungen erfolgreich gespeichert!",
            Self::Fr => "Paramètres enregistrés avec succès !",
            Self::It => "Impostazioni salvate con successo!",
            Self::Es => "¡Configuración guardada con éxito!",
            Self::Pt => "Configurações salvas com sucesso!",
            Self::Ja => "設定が正常に保存されました！",
            Self::Ru => "Настройки успешно сохранены!",
            Self::Pl => "Ustawienia zostały pomyślnie zapisane!",
            Self::Zh => "设置已成功保存！",
        }
    }

    // --- Modals & Dialogs ---

    pub fn create_dir_title(&self) -> &'static str {
        match self {
            Self::En => "Create Output Directory?",
            Self::De => "Ausgabe-Ordner erstellen?",
            Self::Fr => "Créer le dossier de sortie ?",
            Self::It => "Creare la cartella di destinazione?",
            Self::Es => "¿Crear carpeta de salida?",
            Self::Pt => "Criar pasta de saída?",
            Self::Ja => "出力フォルダを作成しますか？",
            Self::Ru => "Создать папку назначения?",
            Self::Pl => "Utworzyć katalog wyjściowy?",
            Self::Zh => "创建输出目录？",
        }
    }

    pub fn create_dir_prompt(&self, dir: &str) -> String {
        match self {
            Self::En => format!(
                "The following destination directory does not exist yet:\n\n{}\n\nDo you want to create it now?",
                dir
            ),
            Self::De => format!(
                "Das folgende Zielverzeichnis existiert noch nicht:\n\n{}\n\nMöchten Sie es jetzt erstellen?",
                dir
            ),
            Self::Fr => format!(
                "Le répertoire de destination suivant n'existe pas encore :\n\n{}\n\nVoulez-vous le créer maintenant ?",
                dir
            ),
            Self::It => format!(
                "La seguente cartella di destinazione non esiste ancora:\n\n{}\n\nCrearla adesso?",
                dir
            ),
            Self::Es => format!(
                "El siguiente directorio de destino aún no existe:\n\n{}\n\n¿Desea crearlo ahora?",
                dir
            ),
            Self::Pt => format!(
                "A pasta de destino a seguir ainda não existe:\n\n{}\n\nDeseja criá-la agora?",
                dir
            ),
            Self::Ja => format!(
                "以下の出力先フォルダが存在しません:\n\n{}\n\n今すぐ作成しますか？",
                dir
            ),
            Self::Ru => format!(
                "Следующая папка назначения еще не существует:\n\n{}\n\nСоздать её сейчас?",
                dir
            ),
            Self::Pl => format!(
                "Następujący katalog docelowy jeszcze nie istnieje:\n\n{}\n\nCzy chcesz go utworzyć teraz?",
                dir
            ),
            Self::Zh => format!("以下目标目录尚不存在:\n\n{}\n\n是否立即创建？", dir),
        }
    }

    pub fn create_dir_confirm(&self) -> &'static str {
        match self {
            Self::En => "Create Directory & Continue",
            Self::De => "Ordner erstellen & Fortfahren",
            Self::Fr => "Créer le dossier & Continuer",
            Self::It => "Crea cartella e continua",
            Self::Es => "Crear carpeta y continuar",
            Self::Pt => "Criar pasta e continuar",
            Self::Ja => "作成して続行",
            Self::Ru => "Создать и продолжить",
            Self::Pl => "Utwórz katalog i kontynuuj",
            Self::Zh => "创建目录并继续",
        }
    }

    pub fn cancel(&self) -> &'static str {
        match self {
            Self::En => "Cancel",
            Self::De => "Abbrechen",
            Self::Fr => "Annuler",
            Self::It => "Annulla",
            Self::Es => "Cancelar",
            Self::Pt => "Cancelar",
            Self::Ja => "キャンセル",
            Self::Ru => "Отмена",
            Self::Pl => "Anuluj",
            Self::Zh => "取消",
        }
    }

    pub fn missing_encoder_title(&self) -> &'static str {
        match self {
            Self::En => "Missing Audio Encoder",
            Self::De => "Fehlender externer Audio-Encoder",
            Self::Fr => "Encodeur audio manquant",
            Self::It => "Encoder audio mancante",
            Self::Es => "Codificador de audio no encontrado",
            Self::Pt => "Codificador de áudio não encontrado",
            Self::Ja => "オーディオエンコーダーが見つかりません",
            Self::Ru => "Кодировщик аудио не найден",
            Self::Pl => "Brakujący enkoder audio",
            Self::Zh => "缺少音频编码器",
        }
    }

    pub fn missing_encoder_prompt(&self, format: &str) -> String {
        match self {
            Self::En => format!(
                "The required encoder tool for '{}' is not installed or not in PATH.",
                format
            ),
            Self::De => format!(
                "Der erforderliche Encoder für '{}' ist nicht installiert oder nicht im PATH gefunden worden.",
                format
            ),
            Self::Fr => format!(
                "L'encodeur requis pour '{}' n'est pas installé ou est introuvable dans le PATH.",
                format
            ),
            Self::It => format!(
                "L'encoder richiesto per '{}' non è installato o non è presente nel PATH.",
                format
            ),
            Self::Es => format!(
                "El codificador necesario para '{}' no está instalado o no se encuentra en el PATH.",
                format
            ),
            Self::Pt => format!(
                "O codificador necessário para '{}' não está instalado ou não está no PATH.",
                format
            ),
            Self::Ja => format!(
                "'{}' に必要なエンコーダーツールがインストールされていないか、PATHに含まれていません。",
                format
            ),
            Self::Ru => format!(
                "Необходимый кодировщик для '{}' не установлен или не найден в PATH.",
                format
            ),
            Self::Pl => format!(
                "Wymagany enkoder dla formatu '{}' nie jest zainstalowany lub nie znaleziono go w PATH.",
                format
            ),
            Self::Zh => format!("未安装用于 '{}' 的编码器工具，或未在 PATH 中找到。", format),
        }
    }

    pub fn rip_as_flac(&self) -> &'static str {
        match self {
            Self::En => "Rip as Lossless FLAC instead (Pure Rust, built-in)",
            Self::De => "Stattdessen als verlustfreies FLAC rippen (Pure Rust, integriert)",
            Self::Fr => "Extraire en FLAC sans perte (Pure Rust, intégré)",
            Self::It => "Estrai invece in FLAC senza perdita (Pure Rust, integrato)",
            Self::Es => "Extraer como FLAC sin pérdidas (Pure Rust, integrado)",
            Self::Pt => "Extrair em FLAC sem perdas (Pure Rust, integrado)",
            Self::Ja => "代わりに可逆圧縮 FLAC で取り込む (Pure Rust、追加不要)",
            Self::Ru => "Копировать в FLAC без потерь (Pure Rust, встроен)",
            Self::Pl => "Zamiast tego zgrywaj do bezstratnego FLAC (Pure Rust, wbudowany)",
            Self::Zh => "改用无损 FLAC 抓取 (Pure Rust，内置)",
        }
    }

    pub fn rip_as_wav(&self) -> &'static str {
        match self {
            Self::En => "Rip as Uncompressed WAV instead (Pure Rust, built-in)",
            Self::De => "Stattdessen als unkomprimiertes WAV rippen (Pure Rust, integriert)",
            Self::Fr => "Extraire en WAV non compressé (Pure Rust, intégré)",
            Self::It => "Estrai invece in WAV non compresso (Pure Rust, integrato)",
            Self::Es => "Extraer como WAV sin comprimir (Pure Rust, integrado)",
            Self::Pt => "Extrair em WAV não comprimido (Pure Rust, integrado)",
            Self::Ja => "代わりに非圧縮 WAV で取り込む (Pure Rust、追加不要)",
            Self::Ru => "Копировать в несжатый WAV (Pure Rust, встроен)",
            Self::Pl => "Zamiast tego zgrywaj do nieskompresowanego WAV (Pure Rust, wbudowany)",
            Self::Zh => "改用无损未压缩 WAV 抓取 (Pure Rust，内置)",
        }
    }

    // --- TUI Key Hints & Help ---

    pub fn tui_hints(&self) -> &'static str {
        match self {
            Self::En => {
                "[1-3] Tabs | [r] Rip | [c] Cancel | [Space] Toggle | [a/n] All/None | [f] Format | [b] Bitrate | [o] Dir | [q] Quit"
            }
            Self::De => {
                "[1-3] Tabs | [r] Rippen | [c] Abbrechen | [Leertaste] Track | [a/n] Alle/Keine | [f] Format | [b] Bitrate | [o] Ordner | [q] Beenden"
            }
            Self::Fr => {
                "[1-3] Onglets | [r] Extraire | [c] Annuler | [Espace] Piste | [a/n] Tout/Rien | [f] Format | [b] Débit | [o] Dossier | [q] Quitter"
            }
            Self::It => {
                "[1-3] Schede | [r] Estrai | [c] Annulla | [Spazio] Traccia | [a/n] Tutte/Nessuna | [f] Formato | [b] Bitrate | [o] Cartella | [q] Esci"
            }
            Self::Es => {
                "[1-3] Pestañas | [r] Extraer | [c] Cancelar | [Espacio] Pista | [a/n] Todo/Nada | [f] Formato | [b] Bitrate | [o] Carpeta | [q] Salir"
            }
            Self::Pt => {
                "[1-3] Abas | [r] Extrair | [c] Cancelar | [Espaço] Faixa | [a/n] Todas/Nenhuma | [f] Formato | [b] Bitrate | [o] Pasta | [q] Sair"
            }
            Self::Ja => {
                "[1-3] タブ | [r] 取り込み | [c] 中止 | [Space] 選択 | [a/n] 全選択/解除 | [f] 形式 | [b] 音質 | [o] 出力先 | [q] 終了"
            }
            Self::Ru => {
                "[1-3] Вкладки | [r] Рип | [c] Отмена | [Пробел] Трек | [a/n] Все/Ничего | [f] Формат | [b] Битрейт | [o] Папка | [q] Выход"
            }
            Self::Pl => {
                "[1-3] Zakładki | [r] Zgraj | [c] Anuluj | [Spacja] Ścieżka | [a/n] Wszystko/Brak | [f] Format | [b] Bitrate | [o] Folder | [q] Wyjdź"
            }
            Self::Zh => {
                "[1-3] 标签页 | [r] 抓取 | [c] 取消 | [空格] 切换曲目 | [a/n] 全选/取消 | [f] 格式 | [b] 码率 | [o] 目录 | [q] 退出"
            }
        }
    }

    pub fn tui_help_title(&self) -> &'static str {
        match self {
            Self::En => " 📖 OxiDisc - Keyboard Shortcuts & Navigation ",
            Self::De => " 📖 OxiDisc - Tastaturkürzel & Navigation ",
            Self::Fr => " 📖 OxiDisc - Raccourcis clavier & Navigation ",
            Self::It => " 📖 OxiDisc - Scorciatoie da tastiera & Navigazione ",
            Self::Es => " 📖 OxiDisc - Atajos de teclado y Navegación ",
            Self::Pt => " 📖 OxiDisc - Atalhos de teclado & Navegação ",
            Self::Ja => " 📖 OxiDisc - キーボードショートカット & 操作ガイド ",
            Self::Ru => " 📖 OxiDisc - Сочетания клавиш и Навигация ",
            Self::Pl => " 📖 OxiDisc - Skróty klawiszowe i nawigacja ",
            Self::Zh => " 📖 OxiDisc - 键盘快捷键与操作指南 ",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_from_code() {
        assert_eq!(Language::from_code("en"), Some(Language::En));
        assert_eq!(Language::from_code("en_US.UTF-8"), Some(Language::En));
        assert_eq!(Language::from_code("de"), Some(Language::De));
        assert_eq!(Language::from_code("de_DE.UTF-8"), Some(Language::De));
        assert_eq!(Language::from_code("fr"), Some(Language::Fr));
        assert_eq!(Language::from_code("fr_FR.UTF-8"), Some(Language::Fr));
        assert_eq!(Language::from_code("it"), Some(Language::It));
        assert_eq!(Language::from_code("it_IT.UTF-8"), Some(Language::It));
        assert_eq!(Language::from_code("es"), Some(Language::Es));
        assert_eq!(Language::from_code("es_ES.UTF-8"), Some(Language::Es));
        assert_eq!(Language::from_code("pt"), Some(Language::Pt));
        assert_eq!(Language::from_code("pt_BR.UTF-8"), Some(Language::Pt));
        assert_eq!(Language::from_code("ja"), Some(Language::Ja));
        assert_eq!(Language::from_code("ja_JP.UTF-8"), Some(Language::Ja));
        assert_eq!(Language::from_code("ru"), Some(Language::Ru));
        assert_eq!(Language::from_code("ru_RU.UTF-8"), Some(Language::Ru));
        assert_eq!(Language::from_code("pl"), Some(Language::Pl));
        assert_eq!(Language::from_code("pl_PL.UTF-8"), Some(Language::Pl));
        assert_eq!(Language::from_code("zh"), Some(Language::Zh));
        assert_eq!(Language::from_code("zh_CN.UTF-8"), Some(Language::Zh));
        assert_eq!(Language::from_code("zh_TW.UTF-8"), Some(Language::Zh));
        assert_eq!(Language::from_code("unknown"), None);
    }

    #[test]
    fn test_language_resolve() {
        assert_eq!(Language::resolve("en"), Language::En);
        assert_eq!(Language::resolve("de"), Language::De);
        assert_eq!(Language::resolve("fr"), Language::Fr);
        assert_eq!(Language::resolve("it"), Language::It);
        assert_eq!(Language::resolve("es"), Language::Es);
        assert_eq!(Language::resolve("pt"), Language::Pt);
        assert_eq!(Language::resolve("ja"), Language::Ja);
        assert_eq!(Language::resolve("ru"), Language::Ru);
        assert_eq!(Language::resolve("pl"), Language::Pl);
        assert_eq!(Language::resolve("zh"), Language::Zh);
        // "auto" falls back to detected system language
        let _ = Language::resolve("auto");
    }

    #[test]
    fn test_translations_coverage() {
        for lang in Language::ALL {
            assert!(!lang.tab_rip().is_empty());
            assert!(!lang.tab_settings().is_empty());
            assert!(!lang.tab_help().is_empty());
            assert!(!lang.drive().is_empty());
            assert!(!lang.eject().is_empty());
            assert!(!lang.load_tray().is_empty());
            assert!(!lang.refresh().is_empty());
            assert!(!lang.disc_info().is_empty());
            assert!(!lang.start_ripping().is_empty());
            assert!(!lang.cancel_ripping().is_empty());
            assert!(!lang.create_dir_title().is_empty());
            assert!(!lang.create_dir_prompt("/tmp/test").is_empty());
            assert!(!lang.missing_encoder_title().is_empty());
            assert!(!lang.missing_encoder_prompt("MP3").is_empty());
            assert!(!lang.tui_hints().is_empty());
            assert!(!lang.tui_help_title().is_empty());
        }
    }
}

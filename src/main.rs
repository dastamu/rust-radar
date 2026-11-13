//#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod mygeo;
use mygeo::{kartezjanskie_na_azymut, azymut_na_kartezjanskie}; 

use eframe::egui;
use std::net::UdpSocket;
//use std::sync::mpsc::{channel, Receiver};
use std::sync::mpsc::{Receiver};
use std::thread;
//use std::time::{Instant, Duration};
use std::time::{Instant};
use rand::Rng;
use std::time::Duration;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::f32::consts::{PI,TAU};
use eframe::egui::{Color32, RichText};


#[derive(PartialEq, Clone, Copy)]
#[repr(u8)]
enum SimMode {
    Off = 0,
    Random = 1,
    Circle = 2,
    Random2 = 3,
    Random3 = 4,
    Random4 = 5,
    Circle2 = 6,
    Circle3 = 7,
}

impl SimMode {
    fn label(&self) -> &'static str {
        match self {
            SimMode::Off => "Wył.",
            SimMode::Random => "Los",
            SimMode::Circle => "Okrąg",
            SimMode::Random2 => "Los2",
            SimMode::Random3 => "Los3",
            SimMode::Random4 => "Los4",
            SimMode::Circle2 => "Okrąg2",
            SimMode::Circle3 => "Okrąg3",
        }
    }
}

enum UdpMessage {
    Point(f32, f32),
    Beta(u8),
}

struct CanvasApp {
    // Kanał do odbierania danych z wątków UDP
    //rx: Receiver<(f32, f32)>,
    rx: Receiver<UdpMessage>,
    beta: u8, // ostatnia odebrana wartość azymutu (0-255)
    shared_beta: Arc<AtomicU8>, // Współdzielona wartość beta dla wątków
    // Przechowujemy: (numer_punktu, pozycja, czas_odebrania)
    points: Vec<(u64, egui::Pos2, std::time::Instant)>,
    next_point_id: u64,
    // Pola sterujące widocznością siatki i okręgów
    show_grid: bool,
    show_circles: bool,
    // Etykiety punktów
    show_labels: bool,
    // Pałka
    show_beta: bool,
    // Debug
    show_debug: bool,
    // Czas życia punktów w sekundach
    max_age_secs: f32,
    // Krok siatki i okręgów
    range_step: f32,
    // Pola sterujące symulatorem
    sim_mode: SimMode,
    shared_sim_mode: Arc<AtomicU8>,
    // Siatka biegunowa
    deg_v: i32,
    // Zoom
    zoom: f32,
}

fn send_point(socket: &UdpSocket, x: i32, y: i32) {
    let id: u32 = 0x101;
    let length: u32 = 16;

    let mut packet = [0u8; 16];
    packet[0..4].copy_from_slice(&id.to_be_bytes());
    packet[4..8].copy_from_slice(&length.to_be_bytes());
    packet[8..12].copy_from_slice(&x.to_be_bytes());
    packet[12..16].copy_from_slice(&y.to_be_bytes());

    let _ = socket.send_to(&packet, "127.0.0.1:12121");
}

fn send_beta(socket: &UdpSocket, b: i32) {
    let id: u32 = 0x201;
    let length: u32 = 12;

    let mut packet = [0u8; 12];
    packet[0..4].copy_from_slice(&id.to_be_bytes());
    packet[4..8].copy_from_slice(&length.to_be_bytes());
    packet[8..12].copy_from_slice(&b.to_be_bytes());

    let _ = socket.send_to(&packet, "127.0.0.1:12121");
}

impl CanvasApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        //let (tx, rx) = channel();
        let (tx, rx) = std::sync::mpsc::channel::<UdpMessage>();
        // W egui klonujemy egui_ctx bezpośrednio z CreationContext
        let ctx = cc.egui_ctx.clone();

        // Wątek odbioru danych
        thread::spawn(move || {
            let socket = UdpSocket::bind("127.0.0.1:12121").expect("Nie udało się powiązać portu UDP");
            let mut buf = [0u8; 1024]; //bufor na dane

            println!("Serwer UDP nasłuchuje na porcie 12121...");

            loop {
                if let Ok((amt, _src)) = socket.recv_from(&mut buf) {
                    // Walidacja 1: Sprawdzenie, czy dotarło co najmniej 4 bajty by można było odczytać ID
                    if amt >= 8 {
                        // Dekodowanie ID pakietu (bajty 0..4)
                        let id_bytes: [u8; 4] = buf[0..4].try_into().unwrap();
                        let packet_id = u32::from_be_bytes(id_bytes);

                        // Dekodowanie długości odebranego pakietu (bajty 4..8)
                        let len_bytes: [u8; 4] = buf[4..8].try_into().unwrap();
                        let packet_len = u32::from_be_bytes(len_bytes);

                        match packet_id {
                            0x101 if amt == 16 && packet_len == 16 => {
                                let x_val = i32::from_be_bytes(buf[8..12].try_into().unwrap());
                                let y_val = i32::from_be_bytes(buf[12..16].try_into().unwrap());

                                let _ = tx.send(UdpMessage::Point(x_val as f32, y_val as f32));
                                ctx.request_repaint();
                            }
                            0x201 if amt == 12 && packet_len == 12 => {
                                let beta_raw = u32::from_be_bytes(buf[8..12].try_into().unwrap());
                                // Zabezpieczenie zakresu 0..=255
                                let beta_val = beta_raw.min(255) as u8;

                                let _ = tx.send(UdpMessage::Beta(beta_val));
                                ctx.request_repaint();
                            }
                            _ => {}
                        }
                    }
                }
            }
        });

        let shared_sim_mode = Arc::new(AtomicU8::new(SimMode::Off as u8));
        let thread_sim_mode = Arc::clone(&shared_sim_mode);

        let shared_beta = Arc::new(AtomicU8::new(0));
        let thread_beta = Arc::clone(&shared_beta);

        // Wątek generatora punktów
        thread::spawn(move || {
            let socket = UdpSocket::bind("127.0.0.1:0").expect("Nie udało się utworzyć gniazda nadawczego");
            let mut rng = rand::rng();
            let mut angle: f32 = 0.0;
            let mut angle2: f32 = 0.0;

            loop {
                // Odczytujemy aktualny tryb wybrany w GUI
                let current_mode = thread_sim_mode.load(Ordering::Relaxed);
                //println!("Imit mode {}",current_mode);

                match current_mode {
                    1 => {
                        // 1. Losowe punkty z zakresu [-500,500] km
                        let x: i32 = rng.random_range(-500000..=500000);
                        let y: i32 = rng.random_range(-500000..=500000);
                        send_point(&socket, x, y);
                        thread::sleep(Duration::from_secs_f32(1.0));
                    }
                    2 => {
                        // 2. Dodatkowy tryb: ruch po okręgu o promieniu 300
                        let x = (300000.0 * angle.cos()) as i32;
                        let y = (300000.0 * angle.sin()) as i32;
                        send_point(&socket, x, y);
                        angle -= PI/180.0*2.;
                        thread::sleep(Duration::from_secs_f32(1.0));
                    }
                    3 => {
                        // 1. Losowe punkty z zakresu [0,480] km i [0,360] deg
                        let r: f32 = rng.random_range(0.0..=480000.0);
                        let b: f32 = rng.random_range(0.0..=TAU);

                        let (x, y) = azymut_na_kartezjanskie(r,b);
                        send_point(&socket, x as i32, y as i32);
                        thread::sleep(Duration::from_secs_f32(0.1));
                    }
                    4 => {
                        // 1. Losowe punkty z zakresu [-480,480] km
                        let x: i32 = rng.random_range(-480000..=480000);
                        let y: i32 = rng.random_range(-480000..=480000);
                        send_point(&socket, x, y);
                        thread::sleep(Duration::from_secs_f32(0.01));
                    }
                    5 => {
                        // 1. Losowe punkty z zakresu [-480,480] km
                        let x: i32 = rng.random_range(-480000..=480000);
                        let y: i32 = rng.random_range(-480000..=480000);
                        send_point(&socket, x, y);
                        thread::sleep(Duration::from_secs_f32(0.001));
                    }
                    6 => {
                        // ruch po okręgu o promieniu 300 i 200
                        let x1 = (300000.0 * angle.cos()) as i32;
                        let y1 = (300000.0 * angle.sin()) as i32;
                        send_point(&socket, x1, y1);
                        angle -= PI/180.0*2.;
                        thread::sleep(Duration::from_secs_f32(0.5));

                        let x2 = (200000.0 * angle2.cos()) as i32;
                        let y2 = (200000.0 * angle2.sin()) as i32;
                        send_point(&socket, x2, y2);
                        angle2 += PI/180.0*2.;
                        thread::sleep(Duration::from_secs_f32(0.5));
                    }
                    7 => {
                        // Ruch po okręgu o promieniu 250 wg azymutu z beta
                        let current_beta = thread_beta.load(Ordering::Relaxed);
                        let angle = current_beta as f32 * ((2.*PI)/256.);
                        //println!("Azymut: {} {}",current_beta, angle);
                        let x = (250000.0 * angle.sin()) as i32;
                        let y = (250000.0 * angle.cos()) as i32;
                        send_point(&socket, x, y);
                        thread::sleep(Duration::from_secs_f32(0.0390625));
                    }
                    _ => {
                        // 0. Wyłączony - wątek nic nie wysyła i czeka
                        thread::sleep(Duration::from_secs_f32(1.0));
                    }
                }
            }
        });

        // Wątek pałki
        thread::spawn(move || {
            let socket = UdpSocket::bind("127.0.0.1:0").expect("Nie udało się utworzyć gniazda nadawczego");
            let mut nrbeta: i32 = 0;
            
            loop {
                // wysyłanie sektora
                send_beta(&socket, nrbeta);
                nrbeta += 1;
                if nrbeta >=256 {
                    nrbeta = 0;
                }
                thread::sleep(Duration::from_secs_f32(0.0390625));
                //println!("Nowy azymut {}",nrbeta);
            }
        });

        Self {
            rx,
            beta: 0,
            shared_beta,
            points: Vec::new(),
            next_point_id: 1,   // Zaczynamy numerację od 1
            show_grid: true,    // Domyślnie włączone
            show_circles: true, // Domyślnie włączone
            show_labels: false, // Domyślnie etykiety są niewidoczne
            show_beta: true,    // Domyślnie pałka włączona
            show_debug: false,  // Domyślnie wyłaczony
            max_age_secs: 10.0, // Domyślnie 10 sekund
            range_step: 20.0,   // Domyślnie
            sim_mode: SimMode::Off,
            shared_sim_mode,
            deg_v: 30,          // Domyślnie 30deg
            zoom: 1.0,
        }
    }
}

// Implementacja dostosowana do eframe 0.35 (używamy metody ui zamiast update)
impl eframe::App for CanvasApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let start = Instant::now(); //Pomiar czasu

        /*// Odbieramy nowe punkty i zapisujemy je wraz z aktualnym czasem
        while let Ok(UdpMessage::Point(x, y)) = self.rx.try_recv() {
            self.points.push((self.next_point_id, egui::pos2(x/1000., y/1000.), std::time::Instant::now()));
            self.next_point_id += 1;
        }*/
        // Odbieramy wiadomości z kanału UDP
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                UdpMessage::Point(x, y) => {
                    self.points.push((self.next_point_id, egui::pos2(x/1000., y/1000.), std::time::Instant::now()));
                    self.next_point_id += 1;
                }
                UdpMessage::Beta(b) => {
                    self.beta = b;
                    self.shared_beta.store(b, Ordering::Relaxed);
                }
            }
        }

        // Automatyczne usuwanie punktów starszych
        self.points.retain(|(_, _, timestamp)| {
            timestamp.elapsed() < std::time::Duration::from_secs_f32(self.max_age_secs)
        });

        // Wymuszamy ciągłe przerysowywanie (repaint), aby punkty znikały na bieżąco
        ui.request_repaint();

        // Zmienna, w której zapiszemy tekst statusu kursora
        let mut cursor_status = "X -, Y -".to_string();
        let mut cursor_polar = "R -, Beta -".to_string();

        ui.add_space(5.0); // Margines górny: odsuwa cały wiersz o 5px od samej góry okna 

        ui.horizontal(|ui| {
            ui.add_space(5.0); // Wstawia 5 pikseli pustego miejsca z lewej strony
    
            //ui.heading("Parametry");
            //ui.heading(RichText::new("Parametry").color(Color32::LIGHT_BLUE));
            ui.heading(RichText::new("Parametry").color(Color32::from_rgb(96, 96, 96)));
            ui.separator();

            // Suwak do regulacji czasu (zakres od 1 do 100 sekund)
            ui.label("Historia:");
            ui.spacing_mut().slider_width = 100.0;
            ui.add(egui::Slider::new(&mut self.max_age_secs, 1.0..=100.0).suffix("s"));
            ui.separator();

            // Zasięg
            ui.label("Siatka:");
            egui::ComboBox::from_id_salt("range_selector")
                .selected_text(format!("{:.0}", self.range_step))
                .width(30.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.range_step, 1.0, "1 km");
                    ui.selectable_value(&mut self.range_step, 2.0, "2 km");
                    ui.selectable_value(&mut self.range_step, 5.0, "5 km");
                    ui.selectable_value(&mut self.range_step, 10.0, "10 km");
                    ui.selectable_value(&mut self.range_step, 20.0, "20 km");
                    ui.selectable_value(&mut self.range_step, 50.0, "50 km");
                    ui.selectable_value(&mut self.range_step, 100.0, "100 km");
                });
            ui.separator();

            // Biegunowe
            ui.label("Biegunowe:");
            egui::ComboBox::from_id_salt("range_polar")
                .selected_text(format!("{:.0}", self.deg_v))
                .width(30.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.deg_v, 1, "1°");
                    ui.selectable_value(&mut self.deg_v, 3, "3°");
                    ui.selectable_value(&mut self.deg_v, 5, "5°");
                    ui.selectable_value(&mut self.deg_v, 10, "10°");
                    ui.selectable_value(&mut self.deg_v, 15, "15°");
                    ui.selectable_value(&mut self.deg_v, 30, "30°");
                    ui.selectable_value(&mut self.deg_v, 45, "45°");
                });
            ui.separator();

            // Imitacja punktów
            ui.label("Imit:");
            let prev_mode = self.sim_mode;
            egui::ComboBox::from_id_salt("sim_mode_selector")
                .selected_text(self.sim_mode.label())
                .width(50.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.sim_mode, SimMode::Off, SimMode::Off.label());
                    ui.selectable_value(&mut self.sim_mode, SimMode::Random, SimMode::Random.label());
                    ui.selectable_value(&mut self.sim_mode, SimMode::Circle, SimMode::Circle.label());
                    ui.selectable_value(&mut self.sim_mode, SimMode::Random2, SimMode::Random2.label());
                    ui.selectable_value(&mut self.sim_mode, SimMode::Random3, SimMode::Random3.label());
                    ui.selectable_value(&mut self.sim_mode, SimMode::Random4, SimMode::Random4.label());
                    ui.selectable_value(&mut self.sim_mode, SimMode::Circle2, SimMode::Circle2.label());
                    ui.selectable_value(&mut self.sim_mode, SimMode::Circle3, SimMode::Circle3.label());

                });
            // Jeśli wybrano inny tryb, przekazujemy go natychmiast do wątku w tle
            if prev_mode != self.sim_mode {
                self.shared_sim_mode.store(self.sim_mode as u8, Ordering::Relaxed);
            }

            // Kontener dla elementów po prawej stronie
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // UWAGA: Elementy dodawane są od prawej do lewej!
                ui.add_space(5.0); // Wstawia 5 pikseli pustego miejsca z prawej strony
                // Przycisk czyszczenia
                if ui.button("Wyczyść").clicked() {
                    self.points.clear();
                }
            });
        });
        ui.horizontal(|ui| {
            ui.add_space(5.0); // Wstawia 5 pikseli pustego miejsca z lewej strony
            //ui.heading("Widok");
            ui.heading(RichText::new("Widok").color(Color32::from_rgb(96, 96, 96)));
            ui.separator();

            // Checkboxy do włączania/wyłączania elementów siatki
            ui.checkbox(&mut self.show_grid, "Siatka");
            ui.separator();
            ui.checkbox(&mut self.show_circles, "Okręgi");
            ui.separator();
            ui.checkbox(&mut self.show_labels, "Etykiety");
            ui.separator();
            ui.checkbox(&mut self.show_beta, "Pałka");
            ui.separator();
            ui.checkbox(&mut self.show_debug, "Debug");
            ui.separator();

            // Kontener dla elementów po prawej stronie
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // UWAGA: Elementy dodawane są od prawej do lewej!
                ui.add_space(5.0); // Wstawia 5 pikseli pustego miejsca z prawej strony
                // Zoom
                egui::ComboBox::from_id_salt("range_zoom")
                    .selected_text(format!("{:.3}", 1./self.zoom))
                    .width(30.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.zoom, 128., "1/128");
                        ui.selectable_value(&mut self.zoom, 64., "1/64");
                        ui.selectable_value(&mut self.zoom, 32., "1/32");
                        ui.selectable_value(&mut self.zoom, 16., "1/16");
                        ui.selectable_value(&mut self.zoom, 8., "1/8");
                        ui.selectable_value(&mut self.zoom, 4., "1/4");
                        ui.selectable_value(&mut self.zoom, 2., "1/2");
                        ui.selectable_value(&mut self.zoom, 1., "1");
                        ui.selectable_value(&mut self.zoom, 0.5, "2");
                        ui.selectable_value(&mut self.zoom, 0.25, "4");
                        ui.selectable_value(&mut self.zoom, 0.125, "8");
                    });
                ui.label("Zoom:");
            });
        });

        ui.separator();

        // Pobieramy pełną dostępną szerokość i wysokość okna (zostawiając marginesy i miejsce na dolny status)
        let available_w = (ui.available_width() - 10.0).max(100.0);
        let available_h = (ui.available_height() - 40.0).max(100.0);

        // Kanwa wypełnia teraz cały prostokątny obszar
        let desired_size = egui::vec2(available_w, available_h);

        // Wspólną skalę (piksele na jednostkę) wyznaczamy na podstawie krótszego boku
        let min_side = available_w.min(available_h);
        let offset = 15.0; //mały offset od brzegów kanwy
        let scale = ((min_side - offset)/ 1000.0_f32) * self.zoom;

        // Obliczamy maksymalny zakres jednostek matematycznych dla dłuższego boku
        let max_math_x = (available_w / 2.0) / scale;
        let max_math_y = (available_h / 2.0) / scale;
        
        ui.horizontal(|ui| {
            // Dodajemy 5 pikseli pustej przestrzeni z lewej strony
            ui.add_space(5.0);

            // Płótno
            egui::Frame::canvas(ui.style()).show(ui, |ui| {
                //let desired_size = egui::vec2(800.0, 800.0);
                //let (response, painter) = ui.allocate_painter(desired_size, egui::Sense::hover());
                let (response, painter) = ui.allocate_painter(desired_size, egui::Sense::hover());
                let rect = response.rect;

                // Tło płótna
                painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(30, 36, 30));
                
                //let center_pos = rect.min + egui::vec2(400.0, 400.0);
                // Dokładny geometryczny środek prostokątnego płótna to teraz nasze (0, 0)
                let center_pos = rect.center();

                let point_font = egui::FontId::monospace(11.0);

                //let grid_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(50, 50, 50));

                // Zwykła linia siatki (subtelna)
                let minor_stroke = egui::Stroke::new(0.50_f32, egui::Color32::from_rgb(45, 52, 45));
                // Wyróżniona linia siatki co 5. krok (ja minimalnie grubsza i wyraźniejsza)
                let major_stroke = egui::Stroke::new(0.75_f32, egui::Color32::from_rgb(75, 88, 75));
                // Etykiety
                let label_color = egui::Color32::from_rgb(120, 135, 120);
                let label_font = egui::FontId::monospace(9.0);
                // Krok
                let steps_x = (max_math_x / self.range_step).floor() as i32;
                let steps_y = (max_math_y / self.range_step).floor() as i32;

                // Rysowanie siatki kartezjańskiej =================================================
                if self.show_grid {

                    // Pionowe linie siatki (od -steps_x*100 do +steps_x*100)
                    for i in -steps_x..=steps_x {
                        if i == 0 { continue; }
                        let px_x = center_pos.x + (i as f32 * self.range_step * scale);

                        // Wybieramy styl linii: co piąty krok jest wyróżniony
                        let stroke = if i % 5 == 0 { major_stroke } else { minor_stroke };
        
                        painter.line_segment(
                            [egui::pos2(px_x, rect.min.y), egui::pos2(px_x, rect.max.y)],
                            stroke,
                        );
                    }

                    // Poziome linie siatki (od -steps_y*100 do +steps_y*100)
                    for i in -steps_y..=steps_y {
                        if i == 0 { continue; }
                        let px_y = center_pos.y - (i as f32 * self.range_step * scale);

                        // Wybieramy styl linii: co piąty krok jest wyróżniony
                        let stroke = if i % 5 == 0 { major_stroke } else { minor_stroke };

                        painter.line_segment(
                            [egui::pos2(rect.min.x, px_y), egui::pos2(rect.max.x, px_y)],
                            stroke,
                        );
                    }
                }

                // Rysowanie okręgów ===============================================================
                if self.show_circles {
                    // Rysujemy okręgi co 100 aż do krawędzi dłuższego boku
                    let max_r_steps = (max_math_x.max(max_math_y) / self.range_step).floor() as i32 + 2;
                    for r in 1..=max_r_steps {
                        let pixel_radius = (r as f32 * self.range_step) * scale;
                        // Wybieramy styl linii: co piąty krok jest wyróżniony
                        let stroke = if r % 5 == 0 { major_stroke } else { minor_stroke };
                        painter.circle_stroke(center_pos, pixel_radius, stroke);
                    }
                }

                // Główne osie XY ==================================================================
                let axis_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 80));
                // Oś X (pozioma linia środkowa)
                painter.line_segment(
                    [egui::pos2(rect.min.x, center_pos.y), egui::pos2(rect.max.x, center_pos.y)],
                    axis_stroke,
                );
                // Oś Y (pionowa linia środkowa)
                painter.line_segment(
                    [egui::pos2(center_pos.x, rect.min.y), egui::pos2(center_pos.x, rect.max.y)],
                    axis_stroke,
                );

                // Rysowanie promieni ==============================================================
                let radius = response.rect.width().min(response.rect.height()) * 0.50;
                //let stroke = egui::Stroke::new(2.0, egui::Color32::LIGHT_BLUE);
                //let deg_v = 10;
                let deg_l= 360/self.deg_v;
                for i in 0..deg_l {
                    let angle_deg = (i as f32) * (self.deg_v as f32);
                    let angle_rad = angle_deg.to_radians();

                    // Vec2::angled tworzy wektor jednostkowy dla podanego kąta w radianach
                    let direction = egui::Vec2::angled(angle_rad);
                    let end_point = center_pos + direction * radius;

                    painter.line_segment([center_pos, end_point], axis_stroke);
                }

                // Rysowanie pałki azymutu ========================================================= AZ
                if self.show_beta {
                    let angle_deg = self.beta as f32 * (360./256.);
                    //let angle_deg = 0. as f32;
                    let angle_rad = (angle_deg - 90.0).to_radians(); //wg azymutu 0° to kierunek N
                    //println!(" AZ: {} {}",angle_deg,angle_rad);
                    let direction = egui::Vec2::angled(angle_rad);
                    let end_point = center_pos + direction * radius;
                    let beta_stroke = egui::Stroke::new(25.0, egui::Color32::from_rgba_unmultiplied(120, 160, 120, 32));
                    painter.line_segment([center_pos, end_point], beta_stroke);
                }

                // Martwy stożek ===================================================================
                //let fill_color = egui::Color32::from_rgb(220, 50, 50);
                // Na środku rysujemy okrąg wypełniony kolorem
                painter.circle_filled(center_pos, self.range_step*scale, egui::Color32::from_rgb(30, 36, 30));

                // Etykiety osi (co 5 krok) ========================================================
                if self.show_grid || self.show_circles {
                    // Etykiety wzdłuż osi X
                    for i in -steps_x..=steps_x {
                        if i == 0 || i % 5 != 0 { continue; }
                        let val = i as f32 * self.range_step;
                        let px_x = center_pos.x + (val * scale);
                        
                        let mut offset = 4.0; 
                        if val < 0.0 {
                            offset = -12.0;
                        }

                        painter.text(
                            egui::pos2(px_x, center_pos.y + offset),
                            egui::Align2::CENTER_TOP,
                            format!("{:.0}", val),
                            label_font.clone(),
                            label_color,
                        );
                    }

                    // Etykiety wzdłuż osi Y
                    for i in -steps_y..=steps_y {
                        if i == 0 || i % 5 != 0 { continue; }
                        let val = i as f32 * self.range_step;
                        let px_y = center_pos.y - (val * scale);
                        
                        let mut offset = 4.0; 
                        if val < 0.0 {
                            offset = -25.0;
                        }

                        painter.text(
                            egui::pos2(center_pos.x + offset, px_y),
                            egui::Align2::LEFT_CENTER,
                            format!("{:.0}", val),
                            label_font.clone(),
                            label_color,
                        );
                    }
                }

                // Rysowanie i skalowanie punktów wykryć + blednięcie ==============================
                // Rysowanie punktów z użyciem dynamicznego czasu max_age_secs
                for (id, point, timestamp) in &self.points {
                    let elapsed_secs = timestamp.elapsed().as_secs_f32();
                    
                    // Bezpieczne zapobieganie dzieleniu przez zero i wyliczenie współczynnika na bazie suwaka
                    let life_factor = if self.max_age_secs > 0.0 {
                        (self.max_age_secs - elapsed_secs).max(0.0_f32) / self.max_age_secs
                    } else {
                        0.0
                    };
                    
                    // Kolor kropki (neonowy zielony z zanikaniem)
                    let point_color = egui::Color32::from_rgba_premultiplied(
                        0,
                        (255.0 * life_factor) as u8,
                        (150.0 * life_factor) as u8,
                        (255.0 * life_factor) as u8,
                    );

                    // Kolor etykiety (jasny zielono-biały, zanika razem z punktem)
                    let text_color = egui::Color32::from_rgba_premultiplied(
                        (210.0 * life_factor) as u8,
                        (255.0 * life_factor) as u8,
                        (220.0 * life_factor) as u8,
                        (255.0 * life_factor) as u8,
                    );

                    // Przeliczenie względem środka ekranu
                    let global_pos = egui::pos2(
                        center_pos.x + (point.x * scale),
                        center_pos.y - (point.y * scale),
                    );

                    if rect.contains(global_pos) {
                        // rysowanie punktu wykrycia
                        painter.circle_filled(global_pos, 3.0, point_color);

                        // rysowanie etykiety wykryć ===============================================
                        if self.show_labels {
                            let (r, beta_rad) = kartezjanskie_na_azymut(point.x, point.y);
                            let beta_deg = beta_rad.to_degrees();
                            painter.text(
                                global_pos + egui::vec2(8.0, -6.0),
                                egui::Align2::LEFT_BOTTOM,
                                // format!("#{}\nX: {:.0}\nY: {:.0}", id, point.x, point.y),
                                // format!("#{}\nX,Y {:.1},{:.1}\nR,B {:.1},{:.1}", id, point.x, point.y, r, beta_deg),
                                format!("#{}\n{:.1}km {:.1}km\n{:.1}km {:.1}°", id, point.x, point.y, r, beta_deg),
                                point_font.clone(),
                                text_color,
                            );
                        }
                    }
                }

                //Teksty ===========================================================================
                if self.show_debug {
                    /*painter.text(
                        egui::vec2(8.0, -8.0),
                        egui::Align2::LEFT_BOTTOM,
                        format!("Debug: {}", 1),
                        point_font.clone(),
                        label_color,
                    );*/
                    painter.text(
                        egui::pos2(20.0, 70.0),             // Współrzędne X, Y (w układzie okna)
                        egui::Align2::LEFT_TOP,             // Punkt zakotwiczenia tekstu
                        "Debug:",                            // Treść napisu
                        egui::FontId::proportional(12.0),   // Czcionka i rozmiar (np. 18px)
                        egui::Color32::GRAY,                // Kolor tekstu
                    );
                    painter.text(
                        egui::pos2(15.0, 85.0),             // Współrzędne X, Y (w układzie okna)
                        egui::Align2::LEFT_TOP,             // Punkt zakotwiczenia tekstu
                        format!("Liczba: {}\nSkala: {:.3}\nZoom: {:.3}\nBeta: {}", self.points.len(), scale, self.zoom, self.beta), // Treść napisu
                        egui::FontId::proportional(12.0),   // Czcionka i rozmiar (np. 18px)
                        egui::Color32::GRAY,                // Kolor tekstu
                    );
                }

                // Położenie wskaźnika myszy =======================================================
                /*if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    if rect.contains(mouse_pos) {
                        // Pobieramy pozycję myszy względem lewego górnego rogu płótna (0..800)
                        let local_x = mouse_pos.x - rect.min.x;
                        let local_y = mouse_pos.y - rect.min.y;

                        // Odwrotne mapowanie na wartości matematyczne [-500, 500]
                        let mat_x = (local_x / 0.8) - 500.0;
                        let mat_y = 500.0 - (local_y / 0.8);

                        // Formatujemy tekst do wyświetlenia (zaokrąglamy do liczb całkowitych)
                        cursor_status = format!("(X,Y): {:.0},{:.0}", mat_x, mat_y);
                    }
                }*/
                if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    if rect.contains(mouse_pos) {
                        let mat_x = (mouse_pos.x - center_pos.x) / scale;
                        let mat_y = (center_pos.y - mouse_pos.y) / scale;
                        cursor_status = format!("X {:.1}km, Y {:.1}km", mat_x, mat_y);
                        let (r, beta_rad) = kartezjanskie_na_azymut(mat_x, mat_y);
                        cursor_polar = format!("R {:.1}km, Beta {:.1}°", r, beta_rad.to_degrees());
                    }
                }
            });
        });

        ui.separator();

        let duration = start.elapsed(); //Ile mineło czasu

        ui.horizontal(|ui| {
            ui.add_space(5.0); // Wstawia 5 pikseli pustego miejsca z lewej strony
            //ui.heading("Status");
            ui.heading(RichText::new("Status").color(Color32::from_rgb(96, 96, 96)));
            ui.separator();
            // Licznik, Skala, Duration
            //ui.label(format!("Liczba: {}, Skala: {}, Duration: {}us", self.points.len(), scale, duration.as_micros()));
            //ui.label(format!("Liczba: {}, Skala: {}, FPS: {:.1}", self.points.len(), scale, 1000000./(duration.as_micros() as f32)));
            //ui.separator();
            //ui.label(egui::RichText::new(cursor_status).monospace().strong());
            ui.label(egui::RichText::new(cursor_status).monospace());
            ui.separator();
            ui.label(egui::RichText::new(cursor_polar).monospace());


            // Kontener dla elementów po prawej stronie
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // UWAGA: Elementy dodawane są od prawej do lewej!
                ui.add_space(5.0);
                // ui.label(format!("Liczba: {}, Skala: {}, FPS: {:.1}  ", self.points.len(), scale, 1000000./(duration.as_micros() as f32)));
                ui.label(format!("FPS: {:.1}  ", 1000000./(duration.as_micros() as f32)));
            });

        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([815.0, 880.0])
            .with_min_inner_size([800.0, 895.0]), // Rozmiar minimalny
        ..Default::default()
    };

    eframe::run_native(
        "RRadar",
        options,
        Box::new(|cc| Ok(Box::new(CanvasApp::new(cc)))),
    )
}

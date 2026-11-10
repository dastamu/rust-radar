#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use std::net::UdpSocket;
use std::sync::mpsc::{channel, Receiver};
use std::thread;

struct CanvasApp {
    // Kanał do odbierania współrzędnych z wątku UDP
    rx: Receiver<(f32, f32)>,
    // Lista punktów do narysowania na ekranie
    points: Vec<egui::Pos2>,
    // Pola sterujące widocznością siatki i okręgów
    show_grid: bool,
    show_circles: bool,

}

impl CanvasApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (tx, rx) = channel();
        // W egui 0.37 klonujemy egui_ctx bezpośrednio z CreationContext
        let ctx = cc.egui_ctx.clone();

        thread::spawn(move || {
            let socket = UdpSocket::bind("127.0.0.1:12121").expect("Nie udało się powiązać portu UDP");
            // Oczekujemy dokładnie 16 bajtów danych (4 pola * 4 bajty)
            let mut buf = [0u8; 16];

            println!("Serwer UDP (Protokół 0x101) nasłuchuje na porcie 12121...");

            loop {
                if let Ok((amt, _src)) = socket.recv_from(&mut buf) {
                    // Walidacja 1: Sprawdzenie, czy dotarło co najmniej 16 bajtów
                    if amt == 16 {
                        // Dekodowanie ID pakietu (bajty 0..4)
                        let id_bytes: [u8; 4] = buf[0..4].try_into().unwrap();
                        let packet_id = u32::from_be_bytes(id_bytes);

                        // Walidacja 2: Czy ID zgadza się z oczekiwanym 0x101?
                        if packet_id == 0x101 {
                            // Dekodowanie Długości (bajty 4..8)
                            let len_bytes: [u8; 4] = buf[4..8].try_into().unwrap();
                            let packet_len = u32::from_be_bytes(len_bytes);

                            // Walidacja 3: Sprawdzenie pola długości (zależnie od definicji, np. rozmiar całego pakietu = 16)
                            if packet_len == 16 {
                                // Dekodowanie współrzędnej X (bajty 8..12)
                                let x_bytes: [u8; 4] = buf[8..12].try_into().unwrap();
                                let x_val = i32::from_be_bytes(x_bytes);

                                // Dekodowanie współrzędnej Y (bajty 12..16)
                                let y_bytes: [u8; 4] = buf[12..16].try_into().unwrap();
                                let y_val = i32::from_be_bytes(y_bytes);

                                // Przesyłamy współrzędne do wątku głównego GUI
                                let _ = tx.send((x_val as f32, y_val as f32));
                                ctx.request_repaint();
                            } else {
                                eprintln!("Błąd: Niepoprawna długość w nagłówku pakietu: {}", packet_len);
                            }
                        } else {
                            eprintln!("Błąd: Nieznane ID pakietu: 0x{:X}", packet_id);
                        }
                    }
                }
            }
        });


        Self {
            rx,
            points: Vec::new(),
            show_grid: true,    // Domyślnie włączone
            show_circles: true, // Domyślnie włączone
        }
    }
}

// Implementacja dostosowana do eframe 0.37 (używamy metody ui zamiast update)
impl eframe::App for CanvasApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {

        // Odbieramy wszystkie nowe punkty, które przyszły przez UDP od ostatniej klatki
        while let Ok((x, y)) = self.rx.try_recv() {
            self.points.push(egui::pos2(x, y));
        }

        ui.horizontal(|ui| {
            ui.heading("Parametry");
            ui.separator();

            // Checkboxy do włączania/wyłączania elementów siatki
            ui.checkbox(&mut self.show_grid, "Siatka");
            ui.checkbox(&mut self.show_circles, "Okręgi");
            ui.separator();

            // Przycisk czyszczenia
            if ui.button("Wyczyść").clicked() {
                self.points.clear();
            }
            ui.separator();
        });

        ui.separator();

        // Tworzymy płótno (Canvas) zajmujące całą dostępną przestrzeń
        /*egui::Frame::canvas(ui.style()).show(ui, |ui| {
            let desired_size = ui.available_size();
            let (response, painter) = ui.allocate_painter(desired_size, egui::Sense::hover());
            let rect = response.rect;

            // Rysujemy tło płótna
            painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(30, 36, 30));

            // Rysujemy wszystkie odebrane punkty
            for point in &self.points {
                // Współrzędne przeliczamy relatywnie do lewego górnego rogu płótna
                let global_pos = rect.min + egui::vec2(point.x, point.y);
                
                if rect.contains(global_pos) {
                    painter.circle_filled(
                        global_pos, 
                        4.0, 
                        egui::Color32::from_rgb(0, 255, 150) // neonowy zielony
                    );
                }
            }
        });*/

        egui::Frame::canvas(ui.style()).show(ui, |ui| {
            let desired_size = egui::vec2(800.0, 800.0);
            let (response, painter) = ui.allocate_painter(desired_size, egui::Sense::hover());
            let rect = response.rect;

            // Tło płótna
            painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(30, 36, 30));

            // Osie X i Y krzyżujące się na środku (0,0)
            /*let center_x = rect.min.x + 400.0;
            let center_y = rect.min.y + 400.0;
            // Oś X
            painter.line_segment(
                [egui::pos2(rect.min.x, center_y), egui::pos2(rect.max.x, center_y)],
                egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 60)),
            );
            // Oś Y
            painter.line_segment(
                [egui::pos2(center_x, rect.min.y), egui::pos2(center_x, rect.max.y)],
                egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 60)),
            );*/

            
            let center_pos = rect.min + egui::vec2(400.0, 400.0);
            let grid_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(50, 50, 50));


            // Rysowanie siatki 
            if self.show_grid {
                // Generujemy linie pionowe i poziome dla wartości od -400 do 400 (krok 100)
                // Pomijamy 0, bo tam narysujemy wyraźniejsze osie główne
                for i in (-400..=400).step_by(100) {
                    if i == 0 { continue; }
                    
                    // Przeliczanie matematycznego 'i' na odległość w pikselach od lewej/góry
                    // Krok 100 w układzie [-500, 500] odpowiada dokładnie 80 pikselom (100 * 0.8)
                    let offset = (i + 500) as f32 * 0.8;

                    // Linie pionowe (stałe X)
                    painter.line_segment(
                        [egui::pos2(rect.min.x + offset, rect.min.y), egui::pos2(rect.min.x + offset, rect.max.y)],
                        grid_stroke,
                    );

                    // Linie poziome (stałe Y)
                    painter.line_segment(
                        [egui::pos2(rect.min.x, rect.min.y + offset), egui::pos2(rect.max.x, rect.min.y + offset)],
                        grid_stroke,
                    );
                }
            }

            // Rysowanie okręgów
            if self.show_circles {
                // Rysujemy okręgi o promieniach matematycznych 100, 200, 300, 400, 500 wokół środka
                for r in (100..=500).step_by(100) {
                    // Skalowanie promienia: promień matematyczny 100 = 80 pikseli (100 * 0.8)
                    let pixel_radius = r as f32 * 0.8;
                    
                    painter.circle_stroke(
                        center_pos,
                        pixel_radius,
                        grid_stroke,
                    );
                }
            }
            
            // Główne osie XY
            let axis_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(80, 80, 80));
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

            // Rysowanie i skalowanie punktów wykryć
            for point in &self.points {
                // Matematyczne przesunięcie i skalowanie:
                // Zakres wejściowy x: [-500, 500] -> Mapujemy na piksele: [0, 800]
                // piksel_x = ((x + 500) / 1000) * 800 = (x + 500) * 0.8
                let pixel_x = (point.x + 500.0) * 0.8;

                // Zakres wejściowy y: [-500, 500] -> Mapujemy na piksele: [0, 800] z odwróceniem osi
                // piksel_y = 800 - (((y + 500) / 1000) * 800) = 800 - (y + 500) * 0.8
                let pixel_y = 800.0 - ((point.y + 500.0) * 0.8);

                // Współrzędna globalna na ekranie (dodajemy lewy górny róg płótna `rect.min`)
                let global_pos = rect.min + egui::vec2(pixel_x, pixel_y);

                // Rysujemy punkt (tylko jeśli mieści się na płótnie)
                if rect.contains(global_pos) {
                    painter.circle_filled(
                        global_pos,
                        5.0, // Promień kropki
                        egui::Color32::from_rgb(0, 255, 150),
                    );
                }
            }
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.heading("Status");
            ui.separator();
            // Licznik
            ui.label(format!("Liczba: {}", self.points.len()));
            ui.separator();
        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([825.0, 875.0]),
        ..Default::default()
    };

    eframe::run_native(
        "RRadar",
        options,
        Box::new(|cc| Ok(Box::new(CanvasApp::new(cc)))),
    )
}

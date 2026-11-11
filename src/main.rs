#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use std::net::UdpSocket;
use std::sync::mpsc::{channel, Receiver};
use std::thread;
//use std::time::{Instant, Duration};
use std::time::{Instant};
use rand::Rng;
use std::time::Duration;

struct CanvasApp {
    // Kanał do odbierania współrzędnych z wątku UDP
    rx: Receiver<(f32, f32)>,
    // Przechowujemy: (numer_punktu, pozycja, czas_odebrania)
    points: Vec<(u64, egui::Pos2, std::time::Instant)>,
    next_point_id: u64,
    // Pola sterujące widocznością siatki i okręgów
    show_grid: bool,
    show_circles: bool,
    // Etykiety punktów
    show_labels: bool,
    // Czas życia punktów w sekundach
    max_age_secs: f32,
    // Krok siatki i okręgów
    range_step: f32,
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


        // Generator losowych punktów co 1.0s
        thread::spawn(move || {
            let socket = UdpSocket::bind("127.0.0.1:0").expect("Nie udało się utworzyć gniazda nadawczego");
            let mut rng = rand::rng();

            loop {
                // Losujemy współrzędne z zakresu [-500, 500] włącznie
                let x: i32 = rng.random_range(-500..=500);
                let y: i32 = rng.random_range(-500..=500);

                send_point(&socket, x, y);

                // Czekamy dokładnie 1.0 sekundę przed wysłaniem kolejnego punktu
                thread::sleep(Duration::from_secs_f32(0.1));
            }
        });


        Self {
            rx,
            points: Vec::new(),
            next_point_id: 1,   // Zaczynamy numerację od 1
            show_grid: true,    // Domyślnie włączone
            show_circles: true, // Domyślnie włączone
            show_labels: false, // Domyślnie etykiety są niewidoczne
            max_age_secs: 10.0, // Domyślnie 10 sekund
            range_step: 20.0,   // Domyślnie
        }
    }
}

// Implementacja dostosowana do eframe 0.35 (używamy metody ui zamiast update)
impl eframe::App for CanvasApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let start = Instant::now(); //Pomiar czasu

        // Odbieramy nowe punkty i zapisujemy je wraz z aktualnym czasem
        while let Ok((x, y)) = self.rx.try_recv() {
            self.points.push((self.next_point_id, egui::pos2(x, y), std::time::Instant::now()));
            self.next_point_id += 1;
        }

        // Automatyczne usuwanie punktów starszych
        self.points.retain(|(_, _, timestamp)| {
            timestamp.elapsed() < std::time::Duration::from_secs_f32(self.max_age_secs)
        });

        // Wymuszamy ciągłe przerysowywanie (repaint), aby punkty znikały na bieżąco
        ui.request_repaint();

        // Zmienna, w której zapiszemy tekst statusu kursora
        let mut cursor_status = "Kursor poza płótnem".to_string();

        ui.add_space(5.0); // Margines górny: odsuwa cały wiersz o 5px od samej góry okna 

        ui.horizontal(|ui| {
            ui.add_space(5.0); // Wstawia 5 pikseli pustego miejsca z lewej strony
    
            ui.heading("Parametry");
            ui.separator();

            // Checkboxy do włączania/wyłączania elementów siatki
            ui.checkbox(&mut self.show_grid, "Siatka");
            ui.checkbox(&mut self.show_circles, "Okręgi");
            ui.checkbox(&mut self.show_labels, "Etykiety");
            ui.separator();

            // Suwak do regulacji czasu (zakres od 1 do 100 sekund)
            ui.label("Czas życia:");
            ui.add(egui::Slider::new(&mut self.max_age_secs, 1.0..=100.0).suffix("s"));
            ui.separator();

            // Zasięg
            ui.label("Siatka:");
            egui::ComboBox::from_id_salt("range_selector")
                .selected_text(format!("{:.0}", self.range_step))
                .width(55.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.range_step, 10.0, "10 km");
                    ui.selectable_value(&mut self.range_step, 20.0, "20 km");
                    ui.selectable_value(&mut self.range_step, 50.0, "50 km");
                    ui.selectable_value(&mut self.range_step, 100.0, "100 km");
                });
            ui.separator();

            // Przycisk czyszczenia
            if ui.button("Wyczyść").clicked() {
                self.points.clear();
            }
            ui.separator();
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
        let scale = (min_side - offset)/ 1000.0_f32;

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
                let label_color = egui::Color32::from_rgb(140, 155, 140);
                let label_font = egui::FontId::monospace(8.0);
                // Krok
                let steps_x = (max_math_x / self.range_step).floor() as i32;
                let steps_y = (max_math_y / self.range_step).floor() as i32;

                // Rysowanie siatki 
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

                // Rysowanie okręgów
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

                // Główne osie XY
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
                /*// Osie po skosie
                let axis_thin = egui::Stroke::new(0.5, egui::Color32::from_rgb(80, 80, 80));
                painter.line_segment(
                    [egui::pos2(rect.min.x, rect.min.y), egui::pos2(rect.max.x, rect.max.y)],
                    axis_thin,
                );
                painter.line_segment(
                    [egui::pos2(rect.min.x, rect.max.y), egui::pos2(rect.max.x, rect.min.y)],
                    axis_thin,
                );*/

                // ---- ETYKIETY LICZBOWE NA OSIACH (CO 5. KROK) ----
                if self.show_grid || self.show_circles {
                    // Etykiety wzdłuż osi X
                    for i in -steps_x..=steps_x {
                        if i == 0 || i % 5 != 0 { continue; }
                        let val = i as f32 * self.range_step;
                        let px_x = center_pos.x + (val * scale);
                        
                        painter.text(
                            egui::pos2(px_x, center_pos.y + 4.0),
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
                        
                        painter.text(
                            egui::pos2(center_pos.x + 5.0, px_y),
                            egui::Align2::LEFT_CENTER,
                            format!("{:.0}", val),
                            label_font.clone(),
                            label_color,
                        );
                    }
                }

                // Rysowanie i skalowanie punktów wykryć + blednięcie
                // Rysowanie punktów z użyciem dynamicznego czasu max_age_secs
                for (id, point, timestamp) in &self.points {
                    let elapsed_secs = timestamp.elapsed().as_secs_f32();
                    
                    // Bezpieczne zapobieganie dzieleniu przez zero i wyliczenie współczynnika na bazie suwaka
                    let life_factor = if self.max_age_secs > 0.0 {
                        (self.max_age_secs - elapsed_secs).max(0.0_f32) / self.max_age_secs
                    } else {
                        0.0
                    };
                    
  /*                  // Blaknięcie
                    let r = (0.0 * life_factor) as u8;
                    let g = (255.0 * life_factor) as u8;
                    let b = (150.0 * life_factor) as u8;
                    let a = (255.0 * life_factor) as u8;
                    let point_color = egui::Color32::from_rgba_premultiplied(r, g, b, a);
*/
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

                        // rysowanie etykiety
                        if self.show_labels {
                            painter.text(
                                global_pos + egui::vec2(8.0, -6.0),
                                egui::Align2::LEFT_BOTTOM,
                                format!("#{}\nX: {:.0}\nY: {:.0}", id, point.x, point.y),
                                point_font.clone(),
                                text_color,
                            );
                        }
                    }
                }

                // Położenie wskaźnika myszy
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
                        cursor_status = format!("(X,Y): {:.0},{:.0}", mat_x, mat_y);
                    }
                }
            });
        });

        ui.separator();

        let duration = start.elapsed(); //Ile mineło czasu

        ui.horizontal(|ui| {
            ui.add_space(5.0); // Wstawia 5 pikseli pustego miejsca z lewej strony
            ui.heading("Status");
            ui.separator();
            // Licznik
            ui.label(format!("Liczba: {}, Skala: {}, Duration: {}us", self.points.len(), scale, duration.as_micros()));
            ui.separator();
            ui.label(egui::RichText::new(cursor_status).monospace().strong());
        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([815.0, 880.0])
            .with_min_inner_size([815.0, 880.0]), // Rozmiar minimalny
        ..Default::default()
    };

    eframe::run_native(
        "RRadar",
        options,
        Box::new(|cc| Ok(Box::new(CanvasApp::new(cc)))),
    )
}

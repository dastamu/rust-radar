use std::f32::consts::PI;

#[allow(dead_code)]
pub fn radiany_na_stopnie(x: f32) -> f32 {
    x * (180.0/PI)
}

#[allow(dead_code)]
pub fn stopnie_na_radiany(x: f32) -> f32 {
    x / (180.0/PI)
}

pub fn kartezjanskie_na_biegunowe(x: f32, y: f32) -> (f32, f32) {
    let r = x.hypot(y);      // R = sqrt(x^2 + y^2)
    let mut beta = PI/2. - y.atan2(x);   // Kąt w radianach z przedziału (-PI, PI]
    
    if beta < 0.0 {
        beta = 2.*PI + beta;
    }

    (r, beta)
}

// Ta funkcja jest prywatna i zadziała tylko wewnątrz tego pliku
#[allow(dead_code)]
fn funkcja_pomocnicza() {
    println!("Pomagam!");
}



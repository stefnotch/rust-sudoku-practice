mod all_sudoku_constraints;

use speedy2d::color::Color;
use speedy2d::dimen::Vector2;
use speedy2d::font::{Font, FormattedTextBlock, TextLayout, TextOptions};
use speedy2d::shape::Rectangle;
use speedy2d::window::{VirtualKeyCode, WindowHandler, WindowHelper};
use speedy2d::Graphics2D;
use speedy2d::Window;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;
use std::{cmp, thread};

struct Sudoku {
    fields: [[SudokuField; 9]; 9],
    constraints: Vec<SudokuConstraint>,
}

// Might be interesting as a union
#[derive(Clone, Copy)]
struct SudokuField {
    possible_values: [bool; 9],
    solved_value: i8,
}

struct SudokuConstraint {
    id: usize,
    fields: Vec<(usize, usize)>,
}

impl SudokuConstraint {
    fn has_value(self: &Self, sudoku: &Sudoku, value: i8) -> bool {
        self.fields
            .iter()
            .any(|&(x, y)| sudoku.fields[x][y].solved_value == value)
    }
}

struct MyWindowHandler {
    cell_size: f32,
    sudoku: Sudoku,
    font: Font,
    is_mouse_down: bool,
    selection: HashSet<(usize, usize)>,
    hovered_cell: (usize, usize),
}

fn draw_sudoku(
    handler: &MyWindowHandler,
    sudoku: &Sudoku,
    graphics: &mut Graphics2D,
    digits: [Rc<FormattedTextBlock>; 9],
) {
    let size = handler.cell_size;
    let triple_size = size * 3.;

    for y in 0..9 {
        for x in 0..9 {
            let field = sudoku.fields[x][y];
            for val in 0..9 {
                if field.possible_values[val] {
                    let digit_x = x as f32 + ((val % 3) as f32 / 3.);
                    let digit_y = y as f32 + ((val / 3) as f32 / 3.);

                    graphics.draw_text((digit_x * size, digit_y * size), Color::BLUE, &digits[val]);
                }
                if field.solved_value != -1 {
                    graphics.draw_text(
                        (x as f32 * size, y as f32 * size),
                        Color::RED,
                        &digits[field.solved_value as usize],
                    );
                }
            }
        }
    }

    // Thicc line-chans
    for i in 0..=3 {
        let j = i as f32;
        graphics.draw_line(
            (j * triple_size, 0.),
            (j * triple_size, 3. * triple_size),
            2.0,
            Color::BLACK,
        );

        graphics.draw_line(
            (0., j * triple_size),
            (3. * triple_size, j * triple_size),
            2.0,
            Color::BLACK,
        );
    }

    // Thin intermediate lines
    for k in 0..9 {
        let j = k as f32;
        graphics.draw_line((j * size, 0.), (j * size, 9. * size), 1.0, Color::BLACK);

        graphics.draw_line((0., j * size), (9. * size, j * size), 1.0, Color::BLACK);
    }
}

fn draw_selected_cells(handler: &MyWindowHandler, graphics: &mut Graphics2D) {
    let mut draw_highlighted_cell = |x: f32, y: f32, color: Color| {
        graphics.draw_rectangle(
            Rectangle::new(
                Vector2::new(x * handler.cell_size, y * handler.cell_size),
                Vector2::new((x + 1.) * handler.cell_size, (y + 1.) * handler.cell_size),
            ),
            color,
        );
    };

    draw_highlighted_cell(
        handler.hovered_cell.0 as f32,
        handler.hovered_cell.1 as f32,
        Color::from_rgba(0.2, 1.0, 0.2, 0.3),
    );

    for ele in &handler.selection {
        draw_highlighted_cell(
            ele.0 as f32,
            ele.1 as f32,
            Color::from_rgba(0.2, 0.2, 1.0, 0.3),
        );
    }
}

fn is_solved(field: &SudokuField) -> bool {
    field.possible_values.iter().filter(|&&v| v == true).count() == 1
}

fn solve_simple(sudoku: &mut Sudoku) {
    for constraint in &sudoku.constraints {
        for &(x, y) in &constraint.fields {
            // Find totally solved fields
            {
                let field = &sudoku.fields[x][y];
                if field.solved_value == -1 && is_solved(&field) {
                    let solution = field
                        .possible_values
                        .iter()
                        .enumerate()
                        .find(|&(_, &is_possible)| is_possible)
                        .unwrap()
                        .0 as i8;
                    assert!(!constraint.has_value(&sudoku, solution));
                    sudoku.fields[x][y].solved_value = solution;
                }
            }

            // Clear solved fields
            {
                let field = &mut sudoku.fields[x][y];
                if field.solved_value != -1 {
                    field.possible_values.fill(false);
                }
            }

            let solved_value = sudoku.fields[x][y].solved_value;
            // Apply the super simplistic rules
            if solved_value != -1 {
                // Clear out those "possible values"
                for &(x, y) in constraint.fields.iter() {
                    sudoku.fields[x][y].possible_values[solved_value as usize] = false;
                }
            }
        }
    }

    for constraint in sudoku.constraints.iter().filter(|c| c.fields.len() == 9) {
        // Find values that only appear once in that constraint
        for value in 0..9 {
            if constraint.has_value(sudoku, value as i8) {
                continue;
            }

            if constraint
                .fields
                .iter()
                .map(|&(x, y)| sudoku.fields[x][y])
                .filter(|field| field.possible_values[value] == true)
                .count()
                == 1
            {
                for &(x, y) in &constraint.fields {
                    if sudoku.fields[x][y].possible_values[value] {
                        sudoku.fields[x][y].possible_values.fill(false);
                        sudoku.fields[x][y].solved_value = value as i8;
                    }
                }
            }
        }
    }
}

fn solve_spots_overlap(sudoku: &mut Sudoku) {
    let mut inverse_map: HashMap<(usize, usize), Vec<&SudokuConstraint>> = HashMap::new();

    for constraint in &sudoku.constraints {
        for pos in &constraint.fields {
            // Woah, sexy https://stackoverflow.com/a/51585452/3492994
            inverse_map.entry(pos.clone()).or_default().push(constraint);
        }
    }

    for constraint in sudoku.constraints.iter().filter(|c| c.fields.len() == 9) {
        for number in 0..9 {
            if constraint.has_value(sudoku, number as i8) {
                continue;
            }

            let valid_positions: Vec<&(usize, usize)> = constraint
                .fields
                .iter()
                .filter(|&&(x, y)| sudoku.fields[x][y].possible_values[number] == true)
                .collect();

            if valid_positions.is_empty() {
                continue;
            }

            // We have a number and it has some valid positions. Let's see if there are any other constraints which "overlap".
            if let Some(value) = inverse_map.get(&(valid_positions[0].0, valid_positions[0].1)) {
                for &overlapping_constraint in value
                    .iter()
                    .filter(|c| c.id != constraint.id && c.fields.len() == 9)
                {
                    if overlapping_constraint.has_value(sudoku, number as i8) {
                        continue;
                    }

                    if valid_positions.iter().skip(1).all(|pos| {
                        inverse_map
                            .get(pos)
                            .and_then(|v| v.iter().find(|c| c.id == overlapping_constraint.id))
                            .is_some()
                    }) {
                        // Sweet, we found a super overlapping constraint
                        let other_fields = overlapping_constraint
                            .fields
                            .iter()
                            .filter(|pos| !valid_positions.contains(pos));

                        for &(other_x, other_y) in other_fields {
                            sudoku.fields[other_x][other_y].possible_values[number] = false;
                        }
                    }
                }
            }
        }
    }
}

fn constrain_range<T>(range: Range<T>, min: T, max: T) -> Range<T>
where
    T: Ord,
{
    (cmp::max(range.start, min))..(cmp::min(range.end, max))
}

fn solve_non_orthogonal(sudoku: &mut Sudoku) {
    for i in 0i8..9 {
        for j in 0i8..9 {
            let field = sudoku.fields[i as usize][j as usize];
            if field.solved_value != -1 {
                for v in constrain_range((field.solved_value - 1)..(field.solved_value + 2), 0, 9) {
                    for ii in constrain_range((i - 1)..(i + 2), 0, 9) {
                        sudoku.fields[ii as usize][j as usize].possible_values[v as usize] = false;
                    }
                    for jj in constrain_range((j - 1)..(j + 2), 0, 9) {
                        sudoku.fields[i as usize][jj as usize].possible_values[v as usize] = false;
                    }
                }
            }
        }
    }
}

impl WindowHandler for MyWindowHandler {
    fn on_draw(&mut self, helper: &mut WindowHelper, graphics: &mut Graphics2D) {
        graphics.clear_screen(Color::from_rgb(1.0, 1.0, 1.0));

        let digits = ["1", "2", "3", "4", "5", "6", "7", "8", "9"]
            .map(|digit| self.font.layout_text(digit, 24.0, TextOptions::new()));
        draw_sudoku(&self, &self.sudoku, graphics, digits);
        draw_selected_cells(&self, graphics);
        thread::sleep(Duration::from_millis(400));

        solve_simple(&mut self.sudoku);
        solve_non_orthogonal(&mut self.sudoku);

        solve_spots_overlap(&mut self.sudoku); // TODO: This was borked, huh

        helper.request_redraw();
    }

    fn on_mouse_button_down(
        &mut self,
        helper: &mut WindowHelper<()>,
        button: speedy2d::window::MouseButton,
    ) {
        self.selection.clear();
        self.selection.insert(self.hovered_cell);
        self.is_mouse_down = true;
    }

    fn on_mouse_move(
        &mut self,
        helper: &mut WindowHelper<()>,
        position: speedy2d::dimen::Vector2<f32>,
    ) {
        self.hovered_cell = (
            (position.x / self.cell_size) as usize,
            (position.y / self.cell_size) as usize,
        );

        if self.is_mouse_down {
            self.selection.insert(self.hovered_cell);
        }
    }

    fn on_mouse_button_up(
        &mut self,
        helper: &mut WindowHelper<()>,
        button: speedy2d::window::MouseButton,
    ) {
        self.is_mouse_down = false;
        let mut values: Vec<&(usize, usize)> = self.selection.iter().collect();
        values.sort();
        println!("vec!{:?},", values)
    }

    fn on_key_up(
        &mut self,
        helper: &mut WindowHelper<()>,
        virtual_key_code: Option<speedy2d::window::VirtualKeyCode>,
        scancode: speedy2d::window::KeyScancode,
    ) {
        if self.selection.len() == 1 {
            let selected = self.selection.iter().next();

            if let Some(&(x, y)) = selected {
                let key = virtual_key_code.and_then(|code| match code {
                    VirtualKeyCode::Key1 => Some(1),
                    VirtualKeyCode::Key2 => Some(2),
                    VirtualKeyCode::Key3 => Some(3),
                    VirtualKeyCode::Key4 => Some(4),
                    VirtualKeyCode::Key5 => Some(5),
                    VirtualKeyCode::Key6 => Some(6),
                    VirtualKeyCode::Key7 => Some(7),
                    VirtualKeyCode::Key8 => Some(8),
                    VirtualKeyCode::Key9 => Some(9),
                    _ => None,
                });

                if let Some(key_value) = key {
                    self.sudoku.fields[x][y].possible_values.fill(false);
                    self.sudoku.fields[x][y].solved_value = (key_value - 1) as i8;
                }
            }
        }
    }
}

fn main() {
    let sudoku_constraints = all_sudoku_constraints::get_all()
        .into_iter()
        .enumerate()
        .map(|(id, fields)| SudokuConstraint { id, fields })
        .collect();

    let mut sudoku = Sudoku {
        fields: [[SudokuField {
            possible_values: [true; 9],
            solved_value: -1,
        }; 9]; 9],
        constraints: sudoku_constraints,
    };

    sudoku.fields[3][6].solved_value = 2 - 1; // (-1) because off by one. hehe.

    let bytes = include_bytes!("../assets/NotoSans-Regular.ttf");
    let font = Font::new(bytes).unwrap();

    let window = Window::new_centered("Sudoku", (640, 480)).unwrap();
    window.run_loop(MyWindowHandler {
        cell_size: 50.,
        font,
        sudoku,
        is_mouse_down: false,
        selection: HashSet::new(),
        hovered_cell: (0, 0),
    });
}

pub mod hex;

use color_eyre::eyre::Result;
use glam::{EulerRot, Quat, Vec3};
use hex::{HEX_CENTER, HEX_DIRECTION_VECTORS};
use manifest_dir_macros::directory_relative_path;
use mint::Vector3;
use protostar::{
	application::Application,
	xdg::{get_desktop_files, parse_desktop_file, DesktopFile, Icon, IconType},
};
use stardust_xr_fusion::{
	client::{Client, ClientState, FrameInfo, RootHandler},
	core::values::Transform,
	drawable::{Alignment, Bounds, MaterialParameter, Model, ResourceID, Text, TextFit, TextStyle},
	fields::BoxField,
	node::NodeError,
	node::NodeType,
	spatial::Spatial,
};
use stardust_xr_molecules::{touch_plane::TouchPlane, Grabbable, GrabbableSettings};
use std::f32::consts::PI;
use tween::{QuartInOut, Tweener};

const APP_SIZE: f32 = 0.06;
const PADDING: f32 = 0.005;
const MODEL_SCALE: f32 = 0.03;
const ACTIVATION_DISTANCE: f32 = 0.05;

const CYAN: [f32; 4] = [0.0, 1.0, 1.0, 1.0];
const BTN_SELECTED_COLOR: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
const BTN_COLOR: [f32; 4] = [1.0, 1.0, 0.0, 1.0];

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	color_eyre::install().unwrap();
	tracing_subscriber::fmt()
		.with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
		.pretty()
		.init();
	let (client, event_loop) = Client::connect_with_async_loop().await?;
	client.set_base_prefixes(&[directory_relative_path!("res")]);

	let _root = client.wrap_root(AppHexGrid::new(&client))?;

	tokio::select! {
		_ = tokio::signal::ctrl_c() => (),
		e = event_loop => e??,
	};
	Ok(())
}

struct AppHexGrid {
	apps: Vec<App>,
	button: Button,
}
impl AppHexGrid {
	fn new(client: &Client) -> Self {
		let button = Button::new(client).unwrap();
		let mut desktop_files: Vec<DesktopFile> = get_desktop_files()
			.filter_map(|d| parse_desktop_file(d).ok())
			.filter(|d| !d.no_display)
			.collect();

		desktop_files.sort_by_key(|d| d.clone().name.unwrap_or_default());

		let mut apps = Vec::new();
		let mut radius = 1;
		while !desktop_files.is_empty() {
			let mut hex = HEX_CENTER.add(&HEX_DIRECTION_VECTORS[4].clone().scale(radius));
			for i in 0..6 {
				if desktop_files.is_empty() {
					break;
				};
				for _ in 0..radius {
					if desktop_files.is_empty() {
						break;
					};
					apps.push(
						App::create_from_desktop_file(
							button.grabbable.content_parent(),
							hex.get_coords(),
							desktop_files.pop().unwrap(),
						)
						.unwrap(),
					);
					hex = hex.neighbor(i);
				}
			}
			radius += 1;
		}
		AppHexGrid { apps, button }
	}
}
impl RootHandler for AppHexGrid {
	fn frame(&mut self, info: FrameInfo) {
		self.button.frame(info);
		if self.button.touch_plane.touch_started() {
			self.button
				.model
				.model_part("Hex")
				.unwrap()
				.set_material_parameter("color", MaterialParameter::Color(BTN_SELECTED_COLOR))
				.unwrap();
			for app in &mut self.apps {
				app.toggle();
			}
		} else if self.button.touch_plane.touch_stopped() {
			self.button
				.model
				.model_part("Hex")
				.unwrap()
				.set_material_parameter("color", MaterialParameter::Color(BTN_COLOR))
				.unwrap();
		}
		for app in &mut self.apps {
			app.frame(info);
		}
	}

	fn save_state(&mut self) -> ClientState {
		ClientState::default()
	}
}

struct Button {
	touch_plane: TouchPlane,
	grabbable: Grabbable,
	model: Model,
}
impl Button {
	fn new(client: &Client) -> Result<Self, NodeError> {
		let field = BoxField::create(client.get_root(), Transform::default(), [APP_SIZE; 3])?;
		let grabbable = Grabbable::create(
			client.get_root(),
			Transform::default(),
			&field,
			GrabbableSettings {
				max_distance: 0.01,
				..Default::default()
			},
		)?;
		field.set_spatial_parent(grabbable.content_parent())?;
		let touch_plane = TouchPlane::create(
			grabbable.content_parent(),
			Transform::default(),
			[(APP_SIZE + PADDING) / 2.0; 2],
			(APP_SIZE + PADDING) / 2.0,
			0.0..1.0,
			0.0..1.0,
		)?;

		let model = Model::create(
			grabbable.content_parent(),
			Transform::from_rotation_scale(
				Quat::from_rotation_x(PI / 2.0) * Quat::from_rotation_y(PI),
				[MODEL_SCALE; 3],
			),
			&ResourceID::new_namespaced("protostar", "hexagon/hexagon"),
		)?;
		model
			.model_part("Hex")?
			.set_material_parameter("color", MaterialParameter::Color(BTN_COLOR))?;
		Ok(Button {
			touch_plane,
			grabbable,
			model,
		})
	}

	fn frame(&mut self, info: FrameInfo) {
		let _ = self.grabbable.update(&info);
		if self.grabbable.grab_action().actor_started() {
			let _ = self.touch_plane.set_enabled(false);
		}
		if self.grabbable.grab_action().actor_stopped() {
			let _ = self.touch_plane.set_enabled(true);
		}
		self.touch_plane.update();
	}
}

// Model handling
fn model_from_icon(parent: &Spatial, icon: &Icon) -> Result<Model> {
	match &icon.icon_type {
		IconType::Png => {
			let t = Transform::from_rotation_scale(
				Quat::from_rotation_x(PI / 2.0) * Quat::from_rotation_y(PI),
				[APP_SIZE / 2.0; 3],
			);

			let model = Model::create(
				parent,
				t,
				&ResourceID::new_namespaced("protostar", "hexagon/hexagon"),
			)?;
			model
				.model_part("Hex")?
				.set_material_parameter("color", MaterialParameter::Color(CYAN))?;
			model.model_part("Icon")?.set_material_parameter(
				"diffuse",
				MaterialParameter::Texture(ResourceID::Direct(icon.path.clone())),
			)?;
			Ok(model)
		}
		IconType::Gltf => Ok(Model::create(
			parent,
			Transform::from_scale([0.05; 3]),
			&ResourceID::new_direct(icon.path.clone())?,
		)?),
		_ => panic!("Invalid Icon Type"),
	}
}

pub struct App {
	application: Application,
	parent: Spatial,
	position: Vector3<f32>,
	grabbable: Grabbable,
	_field: BoxField,
	icon: Model,
	label: Option<Text>,
	grabbable_shrink: Option<Tweener<f32, f64, QuartInOut>>,
	grabbable_grow: Option<Tweener<f32, f64, QuartInOut>>,
	grabbable_move: Option<Tweener<f32, f64, QuartInOut>>,
	currently_shown: bool,
}
impl App {
	pub fn create_from_desktop_file(
		parent: &Spatial,
		position: impl Into<Vector3<f32>>,
		desktop_file: DesktopFile,
	) -> Result<Self> {
		let position = position.into();
		let field = BoxField::create(parent, Transform::default(), [APP_SIZE; 3])?;
		let application = Application::create(desktop_file)?;
		let icon = application.icon(128, false);
		let grabbable = Grabbable::create(
			parent,
			Transform::from_position(position),
			&field,
			GrabbableSettings {
				max_distance: 0.01,
				..Default::default()
			},
		)?;
		grabbable.content_parent().set_spatial_parent(parent)?;
		field.set_spatial_parent(grabbable.content_parent())?;
		let icon = icon
			.map(|i| model_from_icon(grabbable.content_parent(), &i))
			.unwrap_or_else(|| {
				Ok(Model::create(
					grabbable.content_parent(),
					Transform::from_rotation_scale(
						Quat::from_rotation_x(PI / 2.0) * Quat::from_rotation_y(PI),
						[APP_SIZE * 0.5; 3],
					),
					&ResourceID::new_namespaced("protostar", "hexagon/hexagon"),
				)?)
			})?;

		let label_style = TextStyle {
			character_height: APP_SIZE * 2.0,
			bounds: Some(Bounds {
				bounds: [1.0; 2].into(),
				fit: TextFit::Wrap,
				bounds_align: Alignment::XCenter | Alignment::YCenter,
			}),
			text_align: Alignment::Center.into(),
			..Default::default()
		};
		let label = application.name().and_then(|name| {
			Text::create(
				&icon,
				Transform::from_position_rotation(
					[0.0, 0.1, -(APP_SIZE * 4.0)],
					Quat::from_rotation_x(PI * 0.5),
				),
				name,
				label_style,
			)
			.ok()
		});
		Ok(App {
			parent: parent.alias(),
			position,
			grabbable,
			_field: field,
			label,
			application,
			icon,
			grabbable_shrink: None,
			grabbable_grow: None,
			grabbable_move: None,
			currently_shown: true,
		})
	}
	pub fn content_parent(&self) -> &Spatial {
		self.grabbable.content_parent()
	}
	pub fn toggle(&mut self) {
		self.grabbable.set_enabled(!self.currently_shown).unwrap();
		if self.currently_shown {
			self.grabbable_move = Some(Tweener::quart_in_out(1.0, 0.0001, 0.25)); //TODO make the scale a parameter
		} else {
			self.icon.set_enabled(true).unwrap();
			if let Some(label) = self.label.as_ref() {
				label.set_enabled(true).unwrap()
			}
			self.grabbable_move = Some(Tweener::quart_in_out(0.0001, 1.0, 0.25));
		}
		self.currently_shown = !self.currently_shown;
	}

	fn frame(&mut self, info: FrameInfo) {
		let _ = self.grabbable.update(&info);

		if let Some(grabbable_move) = &mut self.grabbable_move {
			if !grabbable_move.is_finished() {
				let scale = grabbable_move.move_by(info.delta);
				self.grabbable
					.content_parent()
					.set_position(Some(&self.parent), Vec3::from(self.position) * scale)
					.unwrap();
			} else {
				if grabbable_move.final_value() == 0.0001 {
					self.icon.set_enabled(false).unwrap();
					if let Some(label) = self.label.as_ref() {
						label.set_enabled(false).unwrap()
					}
				}
				self.grabbable_move = None;
			}
		}
		if let Some(grabbable_shrink) = &mut self.grabbable_shrink {
			if !grabbable_shrink.is_finished() {
				let scale = grabbable_shrink.move_by(info.delta);
				self.grabbable
					.content_parent()
					.set_scale(Some(&self.parent), Vector3::from([scale; 3]))
					.unwrap();
			} else {
				self.grabbable
					.content_parent()
					.set_spatial_parent(&self.parent)
					.unwrap();
				if self.currently_shown {
					self.grabbable_grow = Some(Tweener::quart_in_out(0.0001, 1.0, 0.25));
					self.grabbable.cancel_angular_velocity();
					self.grabbable.cancel_linear_velocity();
				}
				self.grabbable_shrink = None;
				self.grabbable
					.content_parent()
					.set_position(Some(&self.parent), self.position)
					.unwrap();
				self.grabbable
					.content_parent()
					.set_rotation(Some(&self.parent), Quat::default())
					.unwrap();
				self.icon
					.set_rotation(
						None,
						Quat::from_rotation_x(PI / 2.0) * Quat::from_rotation_y(PI),
					)
					.unwrap();
			}
		} else if let Some(grabbable_grow) = &mut self.grabbable_grow {
			if !grabbable_grow.is_finished() {
				let scale = grabbable_grow.move_by(info.delta);
				self.grabbable
					.content_parent()
					.set_scale(Some(&self.parent), Vector3::from([scale; 3]))
					.unwrap();
			} else {
				self.grabbable
					.content_parent()
					.set_spatial_parent(&self.parent)
					.unwrap();
				self.grabbable_grow = None;
			}
		} else if self.grabbable.valid() && self.grabbable.grab_action().actor_stopped() {
			self.grabbable_shrink = Some(Tweener::quart_in_out(APP_SIZE * 0.5, 0.0001, 0.25));
			let Ok(distance_future) = self.grabbable
				.content_parent()
				.get_position_rotation_scale(&self.parent)
				 else {return};

			let application = self.application.clone();
			let space = self.content_parent().alias();

			//TODO: split the executable string for the args
			tokio::task::spawn(async move {
				let distance_vector = distance_future.await.ok().unwrap().0;
				let distance = Vec3::from(distance_vector).length_squared();

				if distance > ACTIVATION_DISTANCE {
					let client = space.node().client().unwrap();
					let (_, space_rot, _) = space
						.get_position_rotation_scale(&client.get_root())
						.unwrap()
						.await
						.unwrap();
					let (_, y_rot, _) = Quat::from(space_rot).to_euler(EulerRot::XYZ);
					let _ = space.set_transform(
						Some(client.get_root()),
						Transform::from_rotation_scale(Quat::from_rotation_y(y_rot), [1.0; 3]),
					);
					let _ = application.launch(&space);
				}
			});
		}
	}
}

"""`world_doc_schema.py` defines `world.yaml`, the source of truth for a level's world doc, as Pydantic models.

A `world.yaml` names the level's zones and objects and gives each a shape in pixels of the level's `map.png`
(x to the right, y down). Zones form a tree under one root zone: each zone lists the zones and objects directly
in it, and an object is in the zone its centre is in.

Interface:
    point_px_t: a point on the map picture, (x, y) in pixels.
    Circle, Ellipse, Rect, Polygon: the shapes of zones and objects.
    Shape: any of the four, told apart by its `kind`.
    Zone, WorldObject: a zone and an object, as `world.yaml` gives them.
    World: a whole `world.yaml`; `World.FromYaml` reads and checks one.

Example:
    >>> world = World.FromYaml(Path("lunar_surface/source/world.yaml"), (1862, 1072))
    >>> world.getZonesBreadthFirst()
    ['blue zone', 'middle zone', 'red zone']

Must know:
    Gotchas:
        - Zone and object names share one namespace, so a zone and an object can't have the same name.
"""

from collections import deque
from pathlib import Path
from typing import Annotated, Literal

import yaml
from PIL import ImageDraw
from pydantic import BaseModel, ConfigDict, Field, model_validator

type point_px_t = tuple[float, float]
type bounds_px_t = tuple[float, float, float, float]
type color_t = tuple[int, int, int] | tuple[int, int, int, int] | int


class _Model(BaseModel):
    """`_Model` is the base of every model here: it rejects unknown keys and can't be changed after loading.

    Example:
        >>> class Thing(_Model):
        ...     name: str
    """

    model_config = ConfigDict(extra="forbid", frozen=True)


class Circle(_Model):
    """`Circle` is a round shape on the map picture.

    Attributes:
        kind: Always `circle`.
        centre_px: The circle's centre.
        radius_px: The circle's radius.

    Example:
        >>> Circle(kind="circle", centre_px=(100, 200), radius_px=12).containsPx((105, 200))
        True
    """

    kind: Literal["circle"]
    centre_px: point_px_t
    radius_px: float = Field(gt=0)

    def getCentrePx(self) -> point_px_t:
        """`getCentrePx` gives the circle's centre.

        Example:
            >>> circle.getCentrePx()
        """
        return self.centre_px

    def getBoundsPx(self) -> bounds_px_t:
        """`getBoundsPx` gives the circle's bounding box as (min x, min y, max x, max y).

        Example:
            >>> circle.getBoundsPx()
        """
        x, y = self.centre_px
        r: float = self.radius_px
        return (x - r, y - r, x + r, y + r)

    def containsPx(self, point_px: point_px_t) -> bool:
        """`containsPx` says whether a point is inside the circle or on its edge.

        Args:
            point_px: The point.

        Example:
            >>> circle.containsPx((0, 0))
        """
        dx: float = point_px[0] - self.centre_px[0]
        dy: float = point_px[1] - self.centre_px[1]
        return dx * dx + dy * dy <= self.radius_px * self.radius_px

    def draw(
        self, draw: ImageDraw.ImageDraw, scale: float, fill: color_t | None, outline: color_t | None, width_px: int
    ) -> None:
        """`draw` draws the circle on a picture that is `scale` times the map picture's size.

        Args:
            draw: Where to draw.
            scale: How many of the picture's pixels make one of the map picture's.
            fill: The inside's colour, or None for none.
            outline: The edge's colour, or None for none.
            width_px: The edge's width, in the picture's pixels.

        Example:
            >>> circle.draw(draw, 2, None, (255, 0, 0), 6)
        """
        draw.ellipse([v * scale for v in self.getBoundsPx()], fill=fill, outline=outline, width=width_px)


class Ellipse(_Model):
    """`Ellipse` is an axis-aligned ellipse on the map picture, such as a level's playable area.

    Attributes:
        kind: Always `ellipse`.
        centre_px: The ellipse's centre.
        radii_px: Its half-width and half-height.

    Example:
        >>> Ellipse(kind="ellipse", centre_px=(931, 536), radii_px=(903, 508)).containsPx((100, 536))
        True
    """

    kind: Literal["ellipse"]
    centre_px: point_px_t
    radii_px: tuple[Annotated[float, Field(gt=0)], Annotated[float, Field(gt=0)]]

    def getCentrePx(self) -> point_px_t:
        """`getCentrePx` gives the ellipse's centre.

        Example:
            >>> ellipse.getCentrePx()
        """
        return self.centre_px

    def getBoundsPx(self) -> bounds_px_t:
        """`getBoundsPx` gives the ellipse's bounding box as (min x, min y, max x, max y).

        Example:
            >>> ellipse.getBoundsPx()
        """
        (x, y), (rx, ry) = self.centre_px, self.radii_px
        return (x - rx, y - ry, x + rx, y + ry)

    def containsPx(self, point_px: point_px_t) -> bool:
        """`containsPx` says whether a point is inside the ellipse or on its edge.

        Args:
            point_px: The point.

        Example:
            >>> ellipse.containsPx((0, 0))
        """
        dx: float = (point_px[0] - self.centre_px[0]) / self.radii_px[0]
        dy: float = (point_px[1] - self.centre_px[1]) / self.radii_px[1]
        return dx * dx + dy * dy <= 1

    def draw(
        self, draw: ImageDraw.ImageDraw, scale: float, fill: color_t | None, outline: color_t | None, width_px: int
    ) -> None:
        """`draw` draws the ellipse on a picture that is `scale` times the map picture's size.

        Args:
            draw: Where to draw.
            scale: How many of the picture's pixels make one of the map picture's.
            fill: The inside's colour, or None for none.
            outline: The edge's colour, or None for none.
            width_px: The edge's width, in the picture's pixels.

        Example:
            >>> ellipse.draw(draw, 2, None, (255, 0, 0), 6)
        """
        draw.ellipse([v * scale for v in self.getBoundsPx()], fill=fill, outline=outline, width=width_px)


class Rect(_Model):
    """`Rect` is an axis-aligned rectangle on the map picture.

    Attributes:
        kind: Always `rect`.
        min_px: The corner with the smallest x and y (top left).
        max_px: The corner with the largest x and y (bottom right).

    Example:
        >>> Rect(kind="rect", min_px=(0, 0), max_px=(614, 1072)).containsPx((10, 10))
        True
    """

    kind: Literal["rect"]
    min_px: point_px_t
    max_px: point_px_t

    @model_validator(mode="after")
    def _check_corners(self) -> "Rect":
        """`_check_corners` checks that `max_px` is below and to the right of `min_px`.

        Raises:
            ValueError: `max_px` isn't larger than `min_px` in both x and y.

        Example:
            >>> Rect(kind="rect", min_px=(5, 5), max_px=(1, 1))
            Traceback (most recent call last):
            ValidationError: ...
        """
        if not (self.max_px[0] > self.min_px[0] and self.max_px[1] > self.min_px[1]):
            raise ValueError(f"rect: max_px {self.max_px} must be larger than min_px {self.min_px} in x and y")
        return self

    def getCentrePx(self) -> point_px_t:
        """`getCentrePx` gives the rectangle's centre.

        Example:
            >>> rect.getCentrePx()
        """
        return ((self.min_px[0] + self.max_px[0]) / 2, (self.min_px[1] + self.max_px[1]) / 2)

    def getBoundsPx(self) -> bounds_px_t:
        """`getBoundsPx` gives the rectangle as (min x, min y, max x, max y).

        Example:
            >>> rect.getBoundsPx()
        """
        return (*self.min_px, *self.max_px)

    def containsPx(self, point_px: point_px_t) -> bool:
        """`containsPx` says whether a point is inside the rectangle or on its edge.

        Args:
            point_px: The point.

        Example:
            >>> rect.containsPx((0, 0))
        """
        return self.min_px[0] <= point_px[0] <= self.max_px[0] and self.min_px[1] <= point_px[1] <= self.max_px[1]

    def draw(
        self, draw: ImageDraw.ImageDraw, scale: float, fill: color_t | None, outline: color_t | None, width_px: int
    ) -> None:
        """`draw` draws the rectangle on a picture that is `scale` times the map picture's size.

        Args:
            draw: Where to draw.
            scale: How many of the picture's pixels make one of the map picture's.
            fill: The inside's colour, or None for none.
            outline: The edge's colour, or None for none.
            width_px: The edge's width, in the picture's pixels.

        Example:
            >>> rect.draw(draw, 2, None, (255, 0, 0), 6)
        """
        draw.rectangle([v * scale for v in self.getBoundsPx()], fill=fill, outline=outline, width=width_px)


class Polygon(_Model):
    """`Polygon` is a simple polygon (its edges don't cross) on the map picture.

    Attributes:
        kind: Always `polygon`.
        points_px: The corners, in order around the polygon.

    Example:
        >>> Polygon(kind="polygon", points_px=[(0, 0), (10, 0), (0, 10)]).containsPx((2, 2))
        True
    """

    kind: Literal["polygon"]
    points_px: list[point_px_t] = Field(min_length=3)

    def _get_signed_area_px(self) -> float:
        """`_get_signed_area_px` gives the polygon's area in square pixels, negative if its corners go anticlockwise.

        Example:
            >>> polygon._get_signed_area_px()
        """
        temp_sum: float = 0.0
        for (x0, y0), (x1, y1) in zip(self.points_px, self.points_px[1:] + self.points_px[:1]):
            temp_sum += x0 * y1 - x1 * y0
        return temp_sum / 2

    @model_validator(mode="after")
    def _check_area(self) -> "Polygon":
        """`_check_area` checks that the polygon isn't flat (all its corners on one line).

        Raises:
            ValueError: The polygon has no area.

        Example:
            >>> Polygon(kind="polygon", points_px=[(0, 0), (1, 1), (2, 2)])
            Traceback (most recent call last):
            ValidationError: ...
        """
        if self._get_signed_area_px() == 0:
            raise ValueError(f"polygon: its points {self.points_px} lie on one line, so it has no area")
        return self

    def getCentrePx(self) -> point_px_t:
        """`getCentrePx` gives the polygon's centroid, its centre of area.

        Example:
            >>> polygon.getCentrePx()
        """
        area_px: float = self._get_signed_area_px()
        temp_x: float = 0.0
        temp_y: float = 0.0
        for (x0, y0), (x1, y1) in zip(self.points_px, self.points_px[1:] + self.points_px[:1]):
            cross: float = x0 * y1 - x1 * y0
            temp_x += (x0 + x1) * cross
            temp_y += (y0 + y1) * cross
        return (temp_x / (6 * area_px), temp_y / (6 * area_px))

    def getBoundsPx(self) -> bounds_px_t:
        """`getBoundsPx` gives the polygon's bounding box as (min x, min y, max x, max y).

        Example:
            >>> polygon.getBoundsPx()
        """
        xs: list[float] = [x for x, _ in self.points_px]
        ys: list[float] = [y for _, y in self.points_px]
        return (min(xs), min(ys), max(xs), max(ys))

    def containsPx(self, point_px: point_px_t) -> bool:
        """`containsPx` says whether a point is inside the polygon, by counting edge crossings of a ray.

        Args:
            point_px: The point.

        Example:
            >>> polygon.containsPx((0, 0))

        Must know:
            Limitations:
                - `containsPx` may count a point exactly on an edge as inside or outside.
        """
        x, y = point_px
        is_inside: bool = False
        for (x0, y0), (x1, y1) in zip(self.points_px, self.points_px[1:] + self.points_px[:1]):
            if (y0 > y) != (y1 > y) and x < x0 + (y - y0) * (x1 - x0) / (y1 - y0):
                is_inside = not is_inside
        return is_inside

    def draw(
        self, draw: ImageDraw.ImageDraw, scale: float, fill: color_t | None, outline: color_t | None, width_px: int
    ) -> None:
        """`draw` draws the polygon on a picture that is `scale` times the map picture's size.

        Args:
            draw: Where to draw.
            scale: How many of the picture's pixels make one of the map picture's.
            fill: The inside's colour, or None for none.
            outline: The edge's colour, or None for none.
            width_px: The edge's width, in the picture's pixels.

        Example:
            >>> polygon.draw(draw, 2, None, (255, 0, 0), 6)
        """
        draw.polygon([(x * scale, y * scale) for x, y in self.points_px], fill=fill, outline=outline, width=width_px)


type Shape = Annotated[Circle | Ellipse | Rect | Polygon, Field(discriminator="kind")]


class Zone(_Model):
    """`Zone` is one zone of the world: an area, and the zones and objects directly in it.

    Attributes:
        shape: The zone's area on the map picture.
        description: One or two sentences on where the zone is, for its section of the doc.
        zones: The names of the zones directly in this zone, in the order the doc lists them.
        objects: The names of the objects directly in this zone, in the order the doc lists them.

    Example:
        >>> Zone(shape=rect, description="Blue's end of the level.", zones=[], objects=["blue spawn"])
    """

    shape: Shape
    description: str = Field(min_length=1)
    zones: list[str] = []
    objects: list[str] = []


class WorldObject(_Model):
    """`WorldObject` is one named object of the world, such as a pylon, a wall or a spawn.

    Attributes:
        shape: The object's outline on the map picture; its centre decides which zone it is in.
        description: One sentence on what the object is.
        layout_name: The object's name in the level layout, `common/maps/<level>/map.json`.

    Example:
        >>> WorldObject(shape=circle, description="Where blue's players enter the level.", layout_name="blue spawn")
    """

    shape: Shape
    description: str = Field(min_length=1)
    layout_name: str = Field(min_length=1)


class World(_Model):
    """`World` is a whole `world.yaml`: a level's zone tree and objects.

    Attributes:
        level: The level's name, such as `Lunar Surface`.
        root: The name of the zone that holds the whole level.
        zones: Every zone, by name.
        objects: Every object, by name.

    Example:
        >>> world = World.FromYaml(Path("lunar_surface/source/world.yaml"), (1862, 1072))
    """

    level: str = Field(min_length=1)
    root: str
    zones: dict[str, Zone]
    objects: dict[str, WorldObject]

    @classmethod
    def FromYaml(cls, path: Path, picture_size_px: tuple[int, int]) -> "World":
        """`FromYaml` reads a `world.yaml` and checks it, including that every shape fits on the map picture.

        Args:
            path: The `world.yaml` file.
            picture_size_px: The map picture's (width, height).

        Returns:
            The world.

        Raises:
            pydantic.ValidationError: The file doesn't follow the schema or its tree is wrong.
            ValueError: A shape lies partly outside the map picture.

        Example:
            >>> world = World.FromYaml(Path("lunar_surface/source/world.yaml"), (1862, 1072))
        """
        world: World = cls.model_validate(yaml.safe_load(path.read_text()))
        errors: list[str] = []
        width_px, height_px = picture_size_px
        for kind, things in (("zone", world.zones), ("object", world.objects)):
            for name, thing in things.items():
                min_x, min_y, max_x, max_y = thing.shape.getBoundsPx()
                if min_x < 0 or min_y < 0 or max_x > width_px or max_y > height_px:
                    errors.append(f"{kind} {name!r}: its shape reaches outside the {width_px}x{height_px} map picture")
        if errors:
            raise ValueError(f"{path}:\n" + "\n".join(errors))
        return world

    @model_validator(mode="after")
    def _check_tree(self) -> "World":
        """`_check_tree` checks that the zones form one tree under the root and each object is in its right zone.

        Raises:
            ValueError: With one line per problem: a name used twice or never defined, a zone or object with no
                parent or several, a zone the root can't reach, or an object whose centre isn't in its zone.

        Example:
            >>> World(level="L", root="all", zones={"all": zone}, objects={})
        """
        errors: list[str] = []
        if self.root not in self.zones:
            raise ValueError(f"root {self.root!r} is not one of the zones {sorted(self.zones)}")
        for name in sorted(self.zones.keys() & self.objects.keys()):
            errors.append(f"{name!r} is both a zone and an object; names must be unique")
        parents_by_name: dict[str, list[str]] = {name: [] for name in [*self.zones, *self.objects]}
        for zone_name, zone in self.zones.items():
            for kind, children, defined in (("zone", zone.zones, self.zones), ("object", zone.objects, self.objects)):
                for child in children:
                    if child not in defined:
                        errors.append(f"zone {zone_name!r} lists {kind} {child!r}, which isn't defined")
                    else:
                        parents_by_name[child].append(zone_name)
        for name, parents in parents_by_name.items():
            if name == self.root and parents:
                errors.append(f"root {name!r} is listed in {parents}; the root has no parent")
            elif name != self.root and len(parents) != 1:
                errors.append(f"{name!r} is listed in {len(parents)} zones {parents}; it must be in exactly one")
        reached: set[str] = set(self._walk_breadth_first())
        for name in self.zones.keys() - reached:
            errors.append(f"zone {name!r} can't be reached from the root {self.root!r}")
        if self.zones[self.root].objects:
            errors.append(f"root {self.root!r} lists objects; every object belongs to a zone inside the root")
        if errors:
            raise ValueError("\n".join(errors))
        for zone_name in self._walk_breadth_first():
            errors.extend(self._get_placement_errors(zone_name))
        if errors:
            raise ValueError("\n".join(errors))
        return self

    def _walk_breadth_first(self) -> list[str]:
        """`_walk_breadth_first` lists the zones the root reaches, the root first, each zone once.

        Example:
            >>> world._walk_breadth_first()
            ['Lunar Surface', 'blue zone', 'middle zone', 'red zone']
        """
        order: list[str] = []
        queue: deque[str] = deque([self.root])
        while queue:
            name: str = queue.popleft()
            if name in order or name not in self.zones:
                continue
            order.append(name)
            queue.extend(self.zones[name].zones)
        return order

    def _get_placement_errors(self, zone_name: str) -> list[str]:
        """`_get_placement_errors` checks that a zone's children have their centres in it, and its objects in no sub-zone.

        Args:
            zone_name: The zone whose children to check.

        Returns:
            One message per child in the wrong place.

        Example:
            >>> world._get_placement_errors("blue zone")
            []
        """
        zone: Zone = self.zones[zone_name]
        errors: list[str] = []
        for child in zone.zones:
            if not zone.shape.containsPx(self.zones[child].shape.getCentrePx()):
                errors.append(f"zone {child!r}: its centre is outside its parent zone {zone_name!r}")
        for child in zone.objects:
            centre_px: point_px_t = self.objects[child].shape.getCentrePx()
            if not zone.shape.containsPx(centre_px):
                errors.append(f"object {child!r}: its centre {centre_px} is outside its zone {zone_name!r}")
            for sub_zone in zone.zones:
                if self.zones[sub_zone].shape.containsPx(centre_px):
                    errors.append(f"object {child!r}: its centre is in sub-zone {sub_zone!r}, so list it there")
        return errors

    def getZonesBreadthFirst(self) -> list[str]:
        """`getZonesBreadthFirst` lists every zone but the root, breadth first: the root's zones, then theirs, and so on.

        Example:
            >>> world.getZonesBreadthFirst()
            ['blue zone', 'middle zone', 'red zone']
        """
        return self._walk_breadth_first()[1:]

    def getZonePath(self, zone_name: str) -> list[str]:
        """`getZonePath` lists the zones from just below the root down to `zone_name`.

        Args:
            zone_name: A zone other than the root.

        Example:
            >>> world.getZonePath("blue top")
            ['blue zone', 'blue top']
        """
        parent_by_zone: dict[str, str] = {child: name for name, zone in self.zones.items() for child in zone.zones}
        path: list[str] = [zone_name]
        while parent_by_zone[path[0]] != self.root:
            path.insert(0, parent_by_zone[path[0]])
        return path

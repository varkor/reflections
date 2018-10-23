/// Helper class for dealing with DOM elements.
class Element {
    constructor(element, classes = []) {
        this.element = typeof element === "string" ? document.createElement(element) : element;
        classes.forEach(c => this.element.classList.add(c));
    }

    append(...children) {
        for (const child of children) {
            this.element.appendChild(child.element);
        }
        return this;
    }

    append_to(parent) {
        parent.append(this);
        return this;
    }

    precede(neighbour) {
        neighbour.element.parentNode.insertBefore(this.element, neighbour.element);
        return this;
    }

    follow(neighbour) {
        neighbour.element.parentNode.insertBefore(this.element, neighbour.element.nextSibling);
        return this;
    }

    listen(event, action) {
        this.element.addEventListener(event, event => action(event, this));
        return this;
    }

    add_text(text) {
        this.element.appendChild(document.createTextNode(text));
        return this;
    }

    get id() {
        return this.element.id;
    }

    set id(id) {
        this.element.id = id;
    }

    get class_list() {
        return this.element.classList;
    }

    for_which(action) {
        action(this);
        return this;
    }
}

class ValueElement extends Element {
    constructor(element, classes = []) {
        super(element, classes);
    }

    get value() {
        return this.element.value;
    }

    set value(value) {
        this.element.value = value;
    }

    with_value(value) {
        this.value = value;
        return this;
    }
}

/// `<input>`
class Input extends ValueElement {
    constructor(type = "text", classes = []) {
        super("input", classes);
        this.element.type = type;
    }

    set placeholder(value) {
        this.element.placeholder = value;
    }
}

/// `<input type="range">`
class RangeSlider extends Input {
    constructor(min, value, max, step = null) {
        super("range");
        this.element.min = min;
        this.element.max = max;
        this.element.value = value;
        if (step !== null) {
            this.element.step = step;
        }
    }
}

/// `<input type="checkbox">`
class Checkbox extends Input {
    constructor(checked) {
        super("checkbox");
        this.checked = checked;
    }

    get checked() {
        return this.element.checked;
    }

    set checked(checked) {
        this.element.checked = checked;
    }
}

/// `<canvas>`
class Canvas extends Element {
    constructor(width, height) {
        super("canvas");
        this.context = this.element.getContext("2d");

        [this.width, this.height] = [width, height];
        const dpr = window.devicePixelRatio;
        [this.element.width, this.element.height] = [this.width * dpr, this.height * dpr];
        [this.element.style.width, this.element.style.height] =
            [`${this.width}px`, `${this.height}px`];

        this.clear();
    }

    clear() {
        this.context.fillStyle = "white";
        const dpr = window.devicePixelRatio;
        this.context.fillRect(0, 0, this.width * dpr, this.height * dpr);
    }
}

/// `<label>`
class Label extends Element {
    constructor(text, input = null) {
        super("label");
        this.add_text(text);
        if (input !== null) {
            this.for_input(input);
        }
    }

    for_input(input, follow = false) {
        this.element.setAttribute("for", input.id);
        if (follow) {
            input.follow(this);
        }
        return this;
    }
}

/// `<div>`
class Div extends Element {
    constructor(classes = []) {
        super("div", classes);
    }
}

/// `<div class="parametric-equation">`
class ParametricEquationInput extends Div {
    constructor(action, classes) {
        super(["parametric-equation"]);

        const BASES = ["x", "y"];
        this.components = [];
        for (const [i, v] of BASES.entries()) {
            this.components.push(
                new Input("text", classes)
                    .append_to(this)
                    .for_which(self => self.placeholder = v)
                    .listen("input", (event, self) => {
                        action(event, self);
                    }),
            );
            if (i + 1 < BASES.length) {
                new Element("span", ["divider"]).append_to(this);
            }
        }
    }
}

/// `<select>`
class Select extends ValueElement {
    constructor(options, selected = null) {
        super("select");
        for (const [value, name] of options) {
            new Option(value, name).append_to(this);
        }
        // Pick a default.
        if (selected !== null) {
            this.value = selected;
        }
    }
}

// `<option>`
class Option extends ValueElement {
    constructor(value, text = value) {
        super("option");
        this.value = value;
        this.add_text(text);
    }
}

/// A region of space to be displayed or computed over.
class View {
    constructor(canvas) {
        // The centre of the view in cartesian coördinates.
        [this.x, this.y] = [0, 0];
        // The dimensions in pixels.
        // FIXME: should this be in cartesian coördinates too?
        [this.width, this.height] = [canvas.width, canvas.height];
        // The zoom factor, on a base-2 exponential scale. I.e. 0 is unzoomed; 1 is 2x; -1 is 0.5x.
        this.scale = 0;
    }
}

/// A specialised canvas specifically for drawing graphs in euclidean space.
class Graph extends Canvas {
    constructor(width, height) {
        super(width, height);
        // We flip the context vertically so that (0, 0) is in the bottom left,
        // as is standard in visual representations of cartesian geometry.
        this.context.transform(1, 0, 0, -1, 0, this.element.height);
    }

    plot_points(view, points, connect = false) {
        const dpr = window.devicePixelRatio;
        const RADIUS = 2;

        const vertices = new Path2D();
        const path = new Path2D();

        this.context.beginPath();
        const scale = 2 ** view.scale;
        for (const [px, py] of points) {
            const [x, y] = [
                (px - view.x) * scale + view.width / 2,
                (py - view.y) * scale + view.height / 2,
            ];
            path.lineTo(x * dpr, y * dpr);
            vertices.moveTo(x * dpr, y * dpr);
            vertices.arc(x * dpr, y * dpr, RADIUS / 2 * dpr, 0, 2 * Math.PI);
        }

        if (connect) {
            this.context.save();
            this.context.lineWidth = RADIUS * dpr;
            this.context.stroke(path);
            this.context.restore();
        }

        this.context.fill(vertices);
    }

    plot_equation(view, points) {
        this.plot_points(view, points, true);
    }
}

const PERFORMANCE_MARKERS = {
    START_MARKER: "start-marker",
    DOM_CONTENT_LOAD: "dom-content-load",
    WASM_BINDGEN_CONNECT: "wasm-bindgen-connect",
    WASM_BINDGEN_CALL: "wasm-bindgen-call",
    WASM_BINDGEN_PARSE: "wasm-bindgen-parse",
    CANVAS_RENDER: "canvas-render",
};

/// Responsible for settings up the connection with Rust via `wasm-bindgen`.
class RustWASMContext {
    static connect() {
        const SERVER = "http://0.0.0.0:8000/";
        const PATH = "target/wasm32-unknown-unknown/release/reflections_bg.wasm";
        return window.wasm_bindgen(`${SERVER}${PATH}`).then(() => {
            performance.mark(PERFORMANCE_MARKERS.WASM_BINDGEN_CONNECT);
            window.wasm_bindgen.initialise();
        });
    }
}

class NonaffineReflection {
    constructor(mirror, figure, bindings, view, settings) {
        mirror = mirror.map(eq => new Equation(eq).substitute(bindings));
        figure = figure.map(eq => new Equation(eq).substitute(bindings));
        this.points = new Promise((resolve, reject) => {
            // FIXME: clean this code up.
            const json = window.wasm_bindgen.proof_of_concept(
                view.x,
                view.y,
                figure[0],
                figure[1],
                mirror[0],
                mirror[1],
                settings.get("method"),
                settings.get("draw_normals"),
                settings.get("threshold"),
                settings.get("reflect"),
                bindings.get("γ"),
            );
            performance.mark(PERFORMANCE_MARKERS.WASM_BINDGEN_CALL);
            try {
                const data = JSON.parse(json);
                performance.mark(PERFORMANCE_MARKERS.WASM_BINDGEN_PARSE);
                resolve(data);
            } catch (err) {
                console.error("Failed to parse Rust data:", json, err);
                reject(new Error("failed to parse Rust data"));
            }
        });
    }

    plot(canvas, view, pointer) {
        return this.points.then(data => {
            let [mirror, _, figure, reflection] = data;
            canvas.context.fillStyle = canvas.context.strokeStyle = "hsl(190, 100%, 50%)";
            canvas.plot_equation(view, figure);
            canvas.context.fillStyle = canvas.context.strokeStyle = "hsl(0, 100%, 50%)";
            canvas.plot_equation(view, mirror);
            // if (pointer.drag_origin === null && pointer.x !== null && pointer.y !== null) {
            //     const dpr = window.devicePixelRatio;
            //     const gradient = canvas.context.createRadialGradient(pointer.x * dpr, canvas.height * dpr - pointer.y * dpr, 0 * dpr, pointer.x * dpr, canvas.height * dpr - pointer.y * dpr, 32 * dpr);
            //     gradient.addColorStop(0, "yellow");
            //     gradient.addColorStop(1, "yellow");
            //     gradient.addColorStop(1, "purple");
            //     canvas.context.fillStyle = canvas.context.strokeStyle = gradient;
            // } else {
                canvas.context.fillStyle = canvas.context.strokeStyle = "hsl(280, 100%, 50%)";
            // }
            canvas.plot_points(view, reflection);
            performance.mark(PERFORMANCE_MARKERS.CANVAS_RENDER);
        });
    }
}

/// Straightforward handling of mathematical equations. This will eventually be entirely handled
/// by Rust.
class Equation {
    constructor(string) {
        this.string = string;
    }

    substitute(bindings) {
        for (const [variable, value] of bindings) {
            this.string = this.string.replace(new RegExp(`\\b${variable}\\b`, "g"), `(${value})`);
        }
        return this;
    }

    variables() {
        return new Set(this.string.match(/\b[a-z]\b/g));
    }

    toString() {
        return this.string;
    }
}

/// Debugging tool for measuring performance.
class PerformanceLogger {
    static log(...marks) {
        const next_mark = () => {
            const next = marks.shift();
            if (next === undefined) {
                return null;
            }
            if (performance.getEntriesByName(next).length === 0) {
                console.warn(`No performance entry named \`${next}\` exists.`);
                return null;
            }
            return next;
        }

        for (let start = next_mark(), end; end = next_mark(); start = end) {
            performance.measure(end, start, end);
        }

        const round = duration => {
            const DECIMAL_PLACES = 1;
            const rounding_factor = 10 ** DECIMAL_PLACES;
            return Math.round(duration * rounding_factor) / rounding_factor;
        };
        performance.getEntriesByType("measure").map(entry => {
            console.log(entry.name, round(entry.duration), `(${round(1000 / entry.duration)} fps)`);
        });

        performance.clearMeasures();
        performance.clearMarks();
    }
}

/// A persistent position of a pointer, such as a cursor or touch.
class Pointer {
    constructor(event, offset) {
        this.update(event, offset);
    }

    update(event, offset) {
        this.x = event.pageX - window.scrollX - offset.left;
        this.y = event.pageY - window.scrollY - offset.top;
    }
}

/// Helper class for dealing with DOM elements. Most methods are chainable.
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

/// `ValueElement`s are those with a `value` attribute.
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
    constructor(action, bases, classes) {
        super(["parametric-equation"]);

        this.components = [];
        for (const [i, v] of bases.entries()) {
            this.components.push(
                new Input("text", classes)
                    .append_to(this)
                    .for_which(self => self.placeholder = v)
                    .listen("input", (event, self) => {
                        action(event, self);
                    }),
            );
            if (i + 1 < bases.length) {
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

/// `<option>`
class Option extends ValueElement {
    constructor(value, text = value) {
        super("option");
        this.value = value;
        this.add_text(text);
    }
}

/// A region of space to be displayed or computed over.
///
/// The class `View` mirrors the Rust struct `View` and should be kept in sync.
class View {
    constructor(canvas) {
        // The centre of the view in cartesian coördinates.
        this.origin = [0, 0];
        // The dimensions in pixels.
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

    /// Adjust a point to be positioned correctly with respect to the view.
    static adjust_point(view, [px, py]) {
        const scale = 2 ** view.scale;
        return [
            (px - view.origin[0]) * scale + view.width / 2,
            (py - view.origin[1]) * scale + view.height / 2,
        ];
    }

    plot_points(view, points, connect = false) {
        const dpr = window.devicePixelRatio;
        const RADIUS = 2;

        const vertices = new Path2D();
        const path = new Path2D();

        const scale = 2 ** view.scale;
        for (const [px, py] of points) {
            const [x, y] = [
                (px - view.origin[0]) * scale + view.width / 2,
                (py - view.origin[1]) * scale + view.height / 2,
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

/// Performance markers are used to measure performance of certain calls (like calling to WASM or
/// rendering).
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
    static connect(log_index) {
        const SERVER = "http://0.0.0.0:8000/";
        const PATH = "target/wasm32-unknown-unknown/release/reflections_bg.wasm";
        return window.wasm_bindgen(`${SERVER}${PATH}`).then(() => {
            PerformanceLogger.mark(log_index, PERFORMANCE_MARKERS.WASM_BINDGEN_CONNECT);
            window.wasm_bindgen.initialise();
        });
    }
}

/// Special variables are those with special meaning, affecting something other than the free
/// variables in the equations.
const SPECIAL_VARIABLES = new Map([
    ["transformation", "µ"],
    ["scaling", "σ"],
    ["translation", "τ"],
]);

/// A `NonaffineReflection` represents the data corresponding to (primarily) a sampling of a
/// reflection of an arbitrary parametric equation in another. All the hard work is actually done in
/// Rust; this is a wrapper over the top of the WASM bindings.
class NonaffineReflection {
    /// Compute a reflection (through Rust WASM).
    constructor(mirror, figure, sigma_tau, bindings, view, settings, log_index) {
        /// The class `RenderReflectionArgs` mirrors the Rust struct `RenderReflectionArgs` and
        /// should be kept in sync.
        class RenderReflectionArgs {
            constructor(view, mirror, figure, sigma_tau, method, threshold) {
                this.view = view;
                this.mirror = mirror;
                this.figure = figure;
                this.sigma_tau = sigma_tau;
                this.method = method;
                this.threshold = threshold;
            }
        }

        /// The class `RenderReflectionData` mirrors the Rust struct `RenderReflectionData` and
        /// should be kept in sync.
        class RenderReflectionData {
            constructor(data) {
                this.mirror = data.mirror;
                this.figure = data.figure;
                // `points` contains the entire reflection, including figure and mirror data,
                // whereas `reflection` extracts solely the image points for convenience.
                this.points = data.reflection;
                this.reflection = data.reflection.map(([r,,]) => r);
            }
        }

        mirror = mirror.map(eq => new Equation(eq).substitute(bindings));
        figure = figure.map(eq => new Equation(eq).substitute(bindings));
        sigma_tau = sigma_tau.map(eq => new Equation(eq).substitute(bindings));
        this.log_index = log_index;
        this.data = new Promise((resolve, reject) => {
            const json = window.wasm_bindgen.render_reflection(JSON.stringify(
                new RenderReflectionArgs(
                    view,
                    mirror,
                    figure,
                    sigma_tau,
                    settings.get("method"),
                    parseInt(settings.get("threshold")),
                ),
            ));
            PerformanceLogger.mark(this.log_index, PERFORMANCE_MARKERS.WASM_BINDGEN_CALL);
            try {
                const data = new RenderReflectionData(JSON.parse(json));
                PerformanceLogger.mark(this.log_index, PERFORMANCE_MARKERS.WASM_BINDGEN_PARSE);
                resolve(data);
            } catch (err) {
                reject(new Error("Failed to parse Rust data."));
            }
        });
    }

    /// Plot the mirror, figure and reflection.
    async plot(canvas, view, _settings) {
        function get_CSS_var(name) {
            return window.getComputedStyle(document.documentElement).getPropertyValue(name);
        }

        const data = await this.data;
        canvas.context.fillStyle = canvas.context.strokeStyle = get_CSS_var("--figure-colour");
        canvas.plot_equation(view, data.figure);
        canvas.context.fillStyle = canvas.context.strokeStyle = get_CSS_var("--mirror-colour");
        canvas.plot_equation(view, data.mirror);
        canvas.context.fillStyle = canvas.context.strokeStyle
            = get_CSS_var("--reflection-colour");
        canvas.plot_points(view, data.reflection);
        PerformanceLogger.mark(this.log_index, PERFORMANCE_MARKERS.CANVAS_RENDER);
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

    toJSON() {
        return this.toString();
    }
}

/// Debugging tool for measuring performance.
class PerformanceLogger {
    static start() {
        const index = PerformanceLogger.index++;
        PerformanceLogger.mark(index, PERFORMANCE_MARKERS.START_MARKER);
        return index;
    }

    static mark(index, marker) {
        performance.mark(`${marker}-${index}`);
    }

    static log(index, ...marks) {
        const next_mark = () => {
            const next = marks.shift();
            if (next === undefined) {
                return null;
            }
            const next_index = `${next}-${index}`;
            if (performance.getEntriesByName(next_index).length === 0) {
                console.warn(`No performance entry named \`${next}\` exists.`);
                return null;
            }
            return next_index;
        }

        let start = next_mark();
        for (let end; end = next_mark(); start = end) {
            performance.measure(end, start, end);
            performance.clearMarks(start);
        }
        if (start !== null) {
            performance.clearMarks(start);
        }

        const round = duration => {
            const DECIMAL_PLACES = 1;
            const rounding_factor = 10 ** DECIMAL_PLACES;
            return Math.round(duration * rounding_factor) / rounding_factor;
        };
        const duration = performance.getEntriesByType("measure").reduce((duration, entry) => {
            console.log(
                entry.name.replace(/-\d+$/, ""),
                round(entry.duration),
                `(${round(1000 / entry.duration)} fps)`,
            );
            return duration + entry.duration;
        }, 0);
        console.log(`(${round(1000 / duration)} fps)`);

        performance.clearMeasures();
    }
}
PerformanceLogger.index = 0;

/// A persistent position of a pointer, such as a cursor or touch.
class Pointer {
    constructor(event, offset) {
        this.update(event, offset);
    }

    /// Set the (x, y) components of the pointer in response to an event (like a click or touch).
    update(event, offset) {
        this.x = event.pageX - offset.left;
        this.y = event.pageY - offset.top;
    }
}

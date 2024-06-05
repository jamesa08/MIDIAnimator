/** @type {import('tailwindcss').Config} */
module.exports = {
    mode: "jit",
    content: ["./src/**/*.{js,jsx,ts,tsx}", "./public/index.html"],
    theme: {
        extend: {},
    },
    plugins: [],
    experimental: {
        optimizeUniversalDefaults: true, // https://github.com/tailwindlabs/tailwindcss/discussions/7411
    },
};

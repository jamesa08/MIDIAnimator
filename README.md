<div id="top"></div>

<!-- PROJECT SHIELDS -->

[![Contributors][contributors-shield]][contributors-url]
[![Forks][forks-shield]][forks-url]
[![Stargazers][stars-shield]][stars-url]
[![Issues][issues-shield]][issues-url]
[![GNU License][license-shield]][license-url]
[![mido version][mido-shield]][mido-url]

<!-- PROJECT LOGO -->
<br />
<div align="center">
  <!--
  <a href="">
    <img src="" alt="Logo" width="80" height="80">
  </a>
  -->
  
  <h3 align="center">MIDIAnimator</h3>

  <p align="center">
    Procedurally animate a MIDI file using Blender. 
    <h1><a href="https://github.com/jamesa08/MIDIAnimator/tree/rust-impl?tab=readme-ov-file#future-of-midianimator">⚠️ Read Updates Here 🚧</a></h1>
    <br />
    <a href="https://midianimatordocs.readthedocs.io/en/latest/"><strong>Explore the docs »</strong></a>
    <br />
    <!-- <br />
    <a href="https://github.com/imacj/MIDIAnimator/">View Demo</a>
    · -->
    <a href="https://github.com/imacj/MIDIAnimator/issues">Report Bug</a>
    ·
    <a href="https://github.com/imacj/MIDIAnimator/issues">Request Feature</a>
  </p>
</div>

<!-- TABLE OF CONTENTS -->
<details>
  <summary>Table of Contents</summary>
  <ol>
    <li>
      <a href="#about-the-project">About The Project</a>
      <ul>
        <li><a href="#built-with">Built With</a></li>
      </ul>
    </li>
    <li><a href="#getting-started">Getting Started</a></li>
    <li><a href="#contributing">Contributing</a></li>
    <li><a href="#license">License</a></li>
    <li><a href="#contact">Contact</a></li>
    <li><a href="#acknowledgments">Acknowledgments</a></li>
  </ol>
</details>

<!-- ABOUT THE PROJECT -->

## About The Project

<!-- [![Product Name Screen Shot][product-screenshot]](https://example.com) -->

**MIDI Animator** aims to provide a cohesive, open-source solution to animating instruments using a MIDI file.

Check out the technical demo:

[![Radiant Ensemble](https://img.youtube.com/vi/hW_21_5kVK8/maxresdefault.jpg)](https://www.youtube.com/watch?v=hW_21_5kVK8 "Radiant Ensemble")

<p align="right">(<a href="#top">back to top</a>)</p>

### Built With

- [Python 3.10](https://python.org/)
- [Blender 3.1](https://www.blender.org/download/releases/3-1/)

<p align="right">(<a href="#top">back to top</a>)</p>

<!-- GETTING STARTED -->

## Getting Started

To get started, check out the [Getting Started](https://midianimatordocs.readthedocs.io/en/latest/general/getting_started.html) docs page.
<br>

<p align="right">(<a href="#top">back to top</a>)</p>

<!-- BUILDING THE DOCS  -->

## Building The Docs

1. Clone the repository `git clone https://github.com/imacj/MIDIAnimator.git` and `cd MIDIAnimator/docs`.
2. Run `pip install -r requirements.txt`.
3. Run `make html` to build the HTML docs.
4. Open `index.html` or run `open build/html/index.html` (for Mac users).
5. Before committing, make sure you clean the build folder with `make clean`.

Note: Instead of reStructuredText markdown files, this project uses MyST markdown files. For some basic information on MyST markdown, visit https://myst-parser.readthedocs.io/en/v0.15.1/sphinx/intro.html and https://jupyterbook.org/en/stable/reference/cheatsheet.html.

The API docs are auto-built with `sphinx-autodoc`. If you want to make changes to the API documentation, please find the file you want to edit and edit the documentation string.

**Please open a PR if you want to make changes to the docs.**
<!--

Useful commands:

for building (in docs dir)
make clean && make html

for opening built html
open build/html/index.html

-->
<br>

<p align="right">(<a href="#top">back to top</a>)</p>

<!-- CONTRIBUTING -->

## Contributing

Contributions are what make the open source community such an amazing place to learn, inspire, and create. Any contributions you make are **greatly appreciated**.

If you have a suggestion that would make this better, please fork the repo and create a pull request. You can also simply open an issue with the tag "enhancement".
Don't forget to give the project a star! Thanks again!

1. Fork the Project
2. Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
3. Commit your Changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the Branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

<p align="right">(<a href="#top">back to top</a>)</p>

<!-- LICENSE -->

## License

Distributed under the GNU General Public License (GPLv3) license. <br>
You may freely change and add to a forked repository as you wish, but you may **not** publish this software as closed source. <br>
_See `LICENSE.txt` for more information._<br>

<p align="right">(<a href="#top">back to top</a>)</p>

<!-- CONTACT -->

## Contact

James Alt - [jalt@capital.edu](mailto:jalt@capital.edu)

Project Link: [https://github.com/imacj/MIDIAnimator](https://github.com/imacj/MIDIAnimator)

<p align="right">(<a href="#top">back to top</a>)</p>

<!-- ACKNOWLEDGMENTS -->

## Acknowledgments

Here are some of the development tools I used to create this project.

- [Visual Studio Code](https://code.visualstudio.com)
- [Blender Development Addon](https://marketplace.visualstudio.com/items?itemName=JacquesLucke.blender-development)
- [Fake Blender Python API Module (for code completion)](https://github.com/nutti/fake-bpy-module)
- [Blender Python API Documentation](https://docs.blender.org/api/2.91/)

<p align="right">(<a href="#top">back to top</a>)</p>

<!-- MARKDOWN LINKS & IMAGES -->
<!-- https://www.markdownguide.org/basic-syntax/#reference-style-links -->

[mido-shield]: https://img.shields.io/pypi/v/mido?label=mido
[contributors-shield]: https://img.shields.io/github/contributors/imacj/MIDIAnimator.svg?style=flat
[contributors-url]: https://github.com/imacj/MIDIAnimator/graphs/contributors
[forks-shield]: https://img.shields.io/github/forks/imacj/MIDIAnimator.svg?style=flat
[mido-url]: https://pypi.org/project/mido/
[forks-url]: https://github.com/imacj/MIDIAnimator/network/members
[stars-shield]: https://img.shields.io/github/stars/imacj/MIDIAnimator.svg?style=flat
[stars-url]: https://github.com/imacj/MIDIAnimator/stargazers
[issues-shield]: https://img.shields.io/github/issues/imacj/MIDIAnimator.svg?style=flat
[issues-url]: https://github.com/imacj/MIDIAnimator/issues
[license-shield]: https://img.shields.io/github/license/imacj/MIDIAnimator.svg?style=flat
[license-url]: https://github.com/imacj/MIDIAnimator/blob/master/LICENSE.txt
[product-screenshot]: images/screenshot.png

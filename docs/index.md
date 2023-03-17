[contributors-shield]: https://img.shields.io/github/contributors/imacj/MIDIAnimator.svg?style=flat
[contributors-url]: https://github.com/imacj/MIDIAnimator/graphs/contributors
[forks-shield]: https://img.shields.io/github/forks/imacj/MIDIAnimator.svg?style=flat
[forks-url]: https://github.com/imacj/MIDIAnimator/network/members
[stars-shield]: https://img.shields.io/github/stars/imacj/MIDIAnimator.svg?style=flat
[stars-url]: https://github.com/imacj/MIDIAnimator/stargazers
[issues-shield]: https://img.shields.io/github/issues/imacj/MIDIAnimator.svg?style=flat
[issues-url]: https://github.com/imacj/MIDIAnimator/issues
[license-shield]: https://img.shields.io/github/license/imacj/MIDIAnimator.svg?style=flat
[license-url]: https://github.com/imacj/MIDIAnimator/blob/master/LICENSE.txt
[product-screenshot]: images/screenshot.png

# MIDI Animator

```{note}
This project is under active development. These documents are subject to change, and things may have unexpected results. 

Documentation is unfinished and is always being updated.
```

[![Contributors][contributors-shield]][contributors-url]
[![Forks][forks-shield]][forks-url]
[![Stargazers][stars-shield]][stars-url]
[![Issues][issues-shield]][issues-url]
[![GNU License][license-shield]][license-url]

## About the project:
**MIDI Animator** aims to provide a cohesive, open-source solution to animating instruments using a MIDI file.

```{admonition} Note
<!-- :class: information -->

Currently unsupported:
- Sustaining notes
- Using MIDI CC data & pitchwheel data
- Bones (for rigs). Only object parent trees are supported

```

*You can download an offline version of the docs [here.](https://midianimatordocs.readthedocs.io/_/downloads/en/latest/pdf/)*

## Getting Started:

Check out how to [install](general/installation.md) the project. See {ref}`getting_started` to get started.

## Contributing

Contributions are what make the open source community such an amazing place to learn, inspire, and create. Any contributions you make are **greatly appreciated**.

If you have a suggestion that would make this better, please fork the repo and create a pull request. You can also simply open an issue with the tag "enhancement".
Don't forget to give the project a star! Thanks again!

1. Fork the Project
2. Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
3. Commit your Changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the Branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

## License

Distributed under the GNU General Public License (GPLv3) license. <br>
You may freely change and add to a forked repository as you wish, but you may **not** publish this software as closed source. <br>
_See `LICENSE.txt` for more information._<br>

## Contact

James Alt - [jalt@capital.edu](mailto:jalt@capital.edu)

Project Link: [https://github.com/imacj/MIDIAnimator](https://github.com/imacj/MIDIAnimator)

## Acknowledgments

Here are some of the development tools I used to create this project.

- [Visual Studio Code](https://code.visualstudio.com)
- [Blender Development Addon](https://marketplace.visualstudio.com/items?itemName=JacquesLucke.blender-development)
- [Fake Blender Python API Module (for code completion)](https://github.com/nutti/fake-bpy-module)
- [Blender Python API Documentation](https://docs.blender.org/api/2.91/)

Thank you to David Reed, Professor of Computer Science at Capital University advising this project and assisting with algorithms and data structures.

<!-- ## Table of Contents -->

```{toctree}
:caption: Table of Contents
:maxdepth: 1

general/installation.md
general/getting_started.md
general/breakdown.md
general/animation_types.md
tutorials/tutorial.md
tutorials/adv_tutorial.md
general/future_plans.md
```

```{toctree}
:caption: API Reference
:maxdepth: 1
:glob:

api/*

```
// The automatic source detection stays on. The content adds a directory that .gitignore hides.
module.exports = {
  content: {
    relative: true,
    files: ['./ignored/*.html', './extra'],
  },
}

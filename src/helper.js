const path = require('path')
const fs = require('fs')
const crypto = require('crypto')

exports.withOutExt = (file) => file.split('.').shift()

exports.mkdirRecursively = (dir) => {
    if (!fs.existsSync(dir)) {
        exports.mkdirRecursively(path.join(dir, '..'))
        fs.mkdirSync(dir)
    }
}

exports.generateId = () => `${crypto.randomBytes(16).toString("hex")}`
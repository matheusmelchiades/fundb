const fs = require('fs')
const config = require('../config')

const events = {
    insert: 'insert',
    find: 'find',
    update: 'update',
    delete: 'delete',
    start: 'start',
    end: 'end'
}

const register = (transation, event, data = {}) => {
    const time = new Date().toISOString()
    const log = { time, transation, event, data }

    fs.appendFileSync(config.logPath, `${JSON.stringify(log)}\n`)
}

module.exports = { register, ...events }
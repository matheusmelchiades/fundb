const fs = require('fs')
const config = require('../config')
const Tail = require('tail').Tail

const events = {
    insert: 'insert',
    find: 'find',
    update: 'update',
    delete: 'delete',
    start: 'start',
    end: 'end',
    commit: 'commit',
    checkpoint: 'checkpoint'
}

const register = (transation, event, table = '', data = {}) => {
    const time = new Date().toISOString()
    const log = { time, transation, event, table, data }

    fs.appendFileSync(config.logPath, `${JSON.stringify(log)}\n`)
}

const watch = (cb) => {
    const tail = new Tail(config.logPath)

    tail.on('line', cb)
};

const watchSystem = () => {

    global.system = JSON.parse(fs.readFileSync(config.systemPath));

    fs.watchFile(config.systemPath, () => {
    
        global.system = JSON.parse(fs.readFileSync(config.systemPath));

    });

}

const writeSystem = system => {

    fs.writeFileSync(config.systemPath, JSON.stringify(system, null, 2));

}

const getAll = () => {
    const logsText = fs.readFileSync(config.logPath, config.encoding)
    const logsSplited = logsText.split('\n')

    return logsSplited.filter(log => !!log)
}

const getLogByTransation = (transation) => {
    const logsText = getAll()
    const logsFiltred = logsText.filter(log => log.includes(transation))
    const logsParsed = logsFiltred.map(log => JSON.parse(log))

    return logsParsed
}

const getOpens = () => {
    const logsText = getAll()
    const logsParsed = logsText.map(log => log ? JSON.parse(log) : false)
    const logsOpend = logsParsed.filter(log => log.event === events.start)
    const logsClosed = logsParsed.filter(log => log.event === events.end)
    const logsOnlyOpen = logsOpend.filter((log) => !logsClosed.filter(closed => closed.transation === log.transation).length)
    const allLogsOpen = logsParsed.filter(log => logsOnlyOpen.some(logOpen => logOpen.transation === log.transation))

    return allLogsOpen
}

module.exports = { register, getLogByTransation, getOpens, watchSystem, watch, writeSystem, ...events }
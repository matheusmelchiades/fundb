const fs = require('fs');
const helper = require('./helper');
const logger = require('./logger')

class Sgdb {

    constructor(rootDir) {
        this.transation = helper.generateId()
        this.database = {}
        this.currentTable = ''
        this.rootDir = rootDir || `${__dirname}/tmp/tables`
        this.isWait = false
        this.waitBy = ''
        this.init()
    }

    init() {
        const instance = fs.existsSync(this.rootDir)

        if (!instance)
            helper.mkdirRecursively(this.rootDir)
        else
            this.load()

        this.watch()

        logger.register(this.transation, logger.start)
    }

    exit() {
        this.updateLocks()
        logger.register(this.transation, logger.end)
    }

    updateLocks(event = '', waitBy) {
        const tables = this.listTables()

        tables.map(table => {
            let currentLocks = []
            if (event === logger.deadlock && waitBy) {
                currentLocks = global.system[table].locks.filter(lock => !lock.waitBy)
                global.system[table].locks = currentLocks
            } else {
                currentLocks = global.system[table].locks.filter(lock => lock.transation !== this.transation)
                global.system[table].locks = currentLocks
            }

            logger.writeSystem(global.system)
        })
    }

    watch() {
        logger.watch((data = {}) => {
            const log = JSON.parse(data)

            switch (log.event) {
                case logger.checkpoint:
                    this.transation === log.transation ? this.checkpoint(log) : false
                    break;
                case logger.commit:
                    if (global.handleSystem.stop && global.handleSystem.waitBy !== log.transation) {
                        global.handleSystem.stop = false
                        this.update(global.handleSystem.dataToUpdate, true)
                    } else {
                        this.commit(log);
                    }
                    break;
                case logger.deadlock:
                    if (global.handleSystem.stop && global.handleSystem.waitBy !== log.transation) {
                        global.handleSystem.stop = false
                        this.update(global.handleSystem.dataToUpdate, true)
                    }
                    break;
                default: return;
            }
        })
    }

    load() {
        try {
            const files = fs.readdirSync(this.rootDir);

            files.map(file => {
                const table = helper.withOutExt(file);
                const path = `${this.rootDir}/${file}`;
                const data = fs.readFileSync(path, 'utf-8');

                this.database[table] = { path, data: JSON.parse(data) };
            })
        } catch (err) {
            console.log(err)
            console.log('Erro in Load resources fundb!');
        }
    }

    createTable(table) {
        try {

            const path = `${this.rootDir}/${table}`;
            const data = [];

            fs.writeFileSync(`${path}.json`, '[]');

            this.database[table] = { path, data: data };

            global.system[table] = {
                sequence: 0,
                locks: []
            }

            logger.writeSystem(global.system);

            console.log(`Created Table ${table} with success!`);
        } catch (err) {
            console.log('Error in create table!');
        }
    }

    setCurrentTable(tableName) {
        const tables = Object.keys(this.database)

        if (!tables.includes(tableName)) return false

        this.load();

        this.currentTable = tableName

        return tableName
    }

    getCurrentTable() {
        return this.currentTable
    }

    listTables() {
        try {
            const files = fs.readdirSync(this.rootDir);
            const tables = files.map(helper.withOutExt)
            return tables;
        } catch (err) {
            console.log('Error in Listing tables!')
        }
    }

    insert(dados) {
        try {

            const sequence = global.system[this.currentTable].sequence + 1;

            const data = { id: sequence, ...dados }

            this.database[this.currentTable].data.push(data);

            logger.register(this.transation, logger.insert, this.currentTable, data)

            global.system[this.currentTable].sequence = sequence;

            logger.writeSystem(global.system);

        } catch (err) {
            console.log(err)
            console.log('Error in insert a new register!')
        }
    }

    update(dados, afterLock = false) {
        try {
            const checkLocks = global.system[this.currentTable].locks.filter(lock => lock.transation === this.transation && lock.dataId === dados.id)
            const filterLocks = lock => lock.transation !== this.transation && lock.dataId === dados.id
            const currentLocks = global.system[this.currentTable].locks.filter(filterLocks)
            const deadLocks = global.system[this.currentTable].locks.filter(lock => lock.waitBy && lock.waitBy.transation === this.transation)

            if (!afterLock && deadLocks.length) {
                this.updateLocks(logger.deadlock, deadLocks[0])
                throw Error('DEAD_LOCK')
            }

            if (!checkLocks.length)
                global.system[this.currentTable].locks.push({
                    dataId: dados.id,
                    transation: this.transation,
                    waitBy: currentLocks[0]
                })

            logger.writeSystem(global.system)


            if (currentLocks.length && !afterLock) {

                global.handleSystem = {
                    ...global.handleSystem,
                    stop: true,
                    dataToUpdate: dados,
                    waitBy: currentLocks[0] || 'ERRO'
                }

                global.handleSystem.message()
                return
            }


            const table = this.database[this.currentTable];

            const idx = table.data.findIndex(o => o.id === dados.id);

            if (idx === -1)
                return false

            const data = this.database[this.currentTable].data[idx]

            this.database[this.currentTable].data[idx].description = dados.description

            logger.register(this.transation, logger.update, this.currentTable, data)


            console.log('\n\n       DATA UPDATED \n\n')

        } catch (err) {
            if (err.message === 'DEAD_LOCK') {
                logger.register(this.transation, logger.deadlock, this.currentTable)
                console.log('\n')
                console.log('###############################')
                console.log('       BUM DEAD LOCK !!!!!     ')
                console.log('###############################')
                console.log('\n')
            }
            console.log('\nError in Updtae a register!\n')
        }
    }

    find() {
        try {
            const table = this.database[this.currentTable]

            logger.register(this.transation, logger.find)

            return table.data
        } catch (err) {
            console.log('Error in find data!')
        }
    }

    findById(index = 0) {
        const dataTable = this.database[this.currentTable].data
        const dataFilter = dataTable.filter(data => data.id == index)

        if (dataFilter.length > 2)
            return [dataFilter[0] || {}]

        return dataFilter
    }

    commit(log) {
        if (!log)
            logger.register(this.transation, logger.commit, this.currentTable)
        else {
            let commited = 0
            const dataToUpload = []
            const dataToUpdate = []
            const currentData = this.database[log.table].data
            const transations = logger.getLogByTransation(log.transation)

            for (let i = transations.length; i >= 0; i--) {
                const event = transations[i] ? transations[i].event : {}

                if (event === logger.commit)
                    commited++

                if (commited < 2 && event === logger.insert && log.transation !== this.transation) {

                    dataToUpload.unshift(transations[i].data)
                }

                if (commited < 2 && event === logger.update && log.transation !== this.transation) {

                    dataToUpdate.unshift(transations[i].data)
                }

            }

            if (dataToUpdate.length) {

                dataToUpdate.map(data => {
                    const idx = currentData.findIndex(o => o.id === data.id);
                    this.database[log.table].data[idx].description = data.description
                })

            }

            if (dataToUpload.length) {

                this.database[log.table].data = [...currentData, ...dataToUpload]

            }

            this.updateLocks()

        }
    }

    checkpoint(log = 'checkpoint') {

        if (log === logger.checkpoint)
            logger.register(this.transation, logger.checkpoint, this.currentTable)

        if (log.transation) {
            let check = 0;
            let dataToSave = []
            let dataToUpdload = []
            let isCommit = false
            const table = this.database[log.table]
            const textSaved = fs.readFileSync(table.path, 'utf-8')
            const dataSaved = JSON.parse(textSaved)
            const transations = logger.getLogByTransation(log.transation)

            for (let i = transations.length; i >= 0; i--) {
                const event = transations[i] ? transations[i].event : {}

                if (event === logger.commit && check < 2)
                    isCommit = true

                if (event === logger.checkpoint) {
                    isCommit = false
                    check++
                }

                if (isCommit && event === logger.insert) {
                    dataToSave.unshift(transations[i].data)
                }

                if (isCommit && event === logger.update) {
                    dataToUpdload.unshift(transations[i].data)
                }

            }

            dataToSave = [...dataSaved, ...dataToSave]

            if (dataToUpdload.length) {
                dataToUpdload.map(data => {
                    const idx = dataToSave.findIndex(o => o.id === data.id);

                    dataToSave[idx].description = data.description
                })
            }

            fs.writeFileSync(table.path, JSON.stringify(dataToSave, null, 2));
        }
    }
}

module.exports = Sgdb;

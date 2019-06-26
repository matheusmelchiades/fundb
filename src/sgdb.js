const fs = require('fs');
const helper = require('./helper');
const logger = require('./logger')

class Sgdb {

    constructor(rootDir) {
        this.transation = helper.generateId()
        this.database = {}
        this.currentTable = ''
        this.rootDir = rootDir || `${__dirname}/tmp/tables`
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
        logger.register(this.transation, logger.end)
    }

    watch() {
        logger.watch((data = {}) => {
            const log = JSON.parse(data)

            switch (log.event) {
                case 'checkpoint':
                    this.transation === log.transation ? this.checkpoint(log) : false
                    break;
                case 'commit':
                    this.commit(log);
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
            fs.writeFileSync(`${this.rootDir}/${table}.json`, '[]');

            this.database[table] = {};

            console.log(`Created Table ${table} with success!`);
        } catch (err) {
            console.log('Error in create table!');
        }
    }

    setCurrentTable(tableName) {
        const tables = Object.keys(this.database)

        if (!tables.includes(tableName)) return false

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
            const table = this.database[this.currentTable];
            const data = { id: table.data.length + 1, ...dados }

            this.database[this.currentTable].data.push(data);

            logger.register(this.transation, logger.insert, this.currentTable, dados)

        } catch (err) {
            console.log('Error in insert a new register!')
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
            const currentData = this.database[log.table].data
            const transations = logger.getLogByTransation(log.transation)

            for (let i = transations.length; i >= 0; i--) {
                const event = transations[i] ? transations[i].event : {}

                if (event === logger.commit)
                    commited++

                if (commited < 2 && event === logger.insert && log.transation !== this.transation) {
                    dataToUpload.unshift(transations[i].data)
                }
            }

            this.database[log.table].data = [...currentData, ...dataToUpload].map((data, id) => ({ id: id + 1, ...data }))
        }
    }

    checkpoint(log = 'checkpoint') {

        if (log === logger.checkpoint)
            logger.register(this.transation, logger.checkpoint, this.currentTable)

        if (log.transation) {
            let check = 0;
            let dataToSave = [];
            let isCommit = false;
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
            }

            dataToSave = [...dataSaved, ...dataToSave].map((data, id) => ({ id: id + 1, ...data }))

            fs.writeFileSync(table.path, JSON.stringify(dataToSave, null, 2));
        }
    }

}

module.exports = Sgdb;

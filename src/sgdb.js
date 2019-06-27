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

        // if (global.system[this.currentTable].locks.include(dados.id)) {
        // }

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

    update(dados) {
        try {

            const table = this.database[this.currentTable];
            
            const idx = table.data.findIndex(o => o.id === dados.id);
            
            if (idx === -1)
                return false
            
            const data = this.database[this.currentTable].data[idx]
            
            this.database[this.currentTable].data[idx].descricao = dados.descricao
            
            logger.register(this.transation, logger.update, this.currentTable, data)
            
            global.system[this.currentTable].locks.push({transation: this.transation, dataId: dados.id})
            
            logger.writeSystem(global.system);

        } catch (err) {
            console.log('Error in Updtae a register!')
        }
    }

    wait(transation, cb) {
        this.isWait = true
        this.waitBy = transation

        console.log('\n')
        console.log('IN WAIT')
        console.log('\n')

        while(this.isWait) {

            this.wait = this.wait

            console.log('\n')
            console.log('IN WAIT')
            console.log('\n')

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
                    this.database[log.table].data[idx].descricao = data.descricao
                })

            }

            if (dataToUpload.length) {
                
                this.database[log.table].data = [...currentData, ...dataToUpload]

            }
            
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
                    
                    dataToSave[idx].descricao = data.descricao
                })
            }

            fs.writeFileSync(table.path, JSON.stringify(dataToSave, null, 2));
        }
    }

}

module.exports = Sgdb;

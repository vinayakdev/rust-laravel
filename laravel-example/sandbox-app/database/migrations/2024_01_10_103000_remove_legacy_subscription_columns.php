<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::table('billing_subscriptions', function (Blueprint $table) {
            $table->dropColumn('legacy_plan_code');
        });
    }
};
